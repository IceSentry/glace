use bevy::{
    app::{prelude::*, AppExit},
    ecs::prelude::*,
    input::{
        mouse::{MouseButtonInput, MouseWheel},
        prelude::*,
    },
    window::{prelude::*, WindowCloseRequested},
    winit::WinitWindows,
};

use self::custom_egui_winit::EguiWinitState;
use crate::renderer::{Msaa, WgpuEncoder, WgpuRenderer, WgpuView};

mod custom_egui_winit;

#[derive(Resource)]
pub struct EguiCtxRes(pub egui::Context);

#[derive(Resource)]
pub struct EguiScreenDesciptorRes(pub egui_wgpu::renderer::ScreenDescriptor);

#[derive(Resource)]
pub struct PaintJobs(Vec<egui::ClippedPrimitive>);

pub struct EguiPlugin;
impl Plugin for EguiPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_systems((setup, setup_render_pass))
            .add_system(begin_frame.in_base_set(CoreSet::PreUpdate))
            // .add_system(update_render_pass)
            // .add_system(render)
            .add_system(handle_mouse_events)
            .add_system(on_exit);
    }
}

fn on_exit(
    exit: EventReader<AppExit>,
    window_close: EventReader<WindowCloseRequested>,
    egui_ctx: Res<EguiCtxRes>,
) {
    if !exit.is_empty() || !window_close.is_empty() {
        egui_ctx.0.memory(|mem| {
            std::fs::write(
                "egui.ron",
                ron::ser::to_string_pretty(&mem, ron::ser::PrettyConfig::new())
                    .expect("failed to serialize egui memory"),
            )
            .expect("Failed to write egui memory");
        })
    }
}

fn setup(mut commands: Commands, windows: Query<&Window>) {
    let window = windows.single();
    let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
        size_in_pixels: [window.width() as u32, window.height() as u32],
        pixels_per_point: window.scale_factor() as f32,
    };
    commands.insert_resource(EguiScreenDesciptorRes(screen_descriptor));
    commands.init_resource::<EguiWinitState>();

    let ctx = egui::Context::default();
    if let Ok(mem) = std::fs::read_to_string("egui.ron") {
        let mem: egui::Memory = ron::de::from_str(&mem).expect("Failed to deserialize egui.ron");
        ctx.memory_mut(|memory| {
            memory.clone_from(&mem);
        })
    }

    log::info!("inserting egui ctx");
    commands.insert_resource(EguiCtxRes(ctx));
    commands.insert_resource(PaintJobs(vec![]));
}

#[derive(Resource)]
pub struct EguiRenderer(egui_wgpu::renderer::Renderer);

fn setup_render_pass(world: &mut World) {
    let msaa = world.resource::<Msaa>();
    let renderer = world.resource::<WgpuRenderer>();
    let egui_renderer = egui_wgpu::renderer::Renderer::new(
        &renderer.device,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        None,
        msaa.samples,
    );
    // let pass = egui_wgpu::renderer::RenderPass::new(
    //     &renderer.device,
    //     wgpu::TextureFormat::Bgra8UnormSrgb,
    //     msaa.samples,
    // );
    world.insert_non_send_resource(EguiRenderer(egui_renderer));
}

pub fn update_render_pass(
    // mut rpass: NonSendMut<EguiRenderPassRes>,
    // renderer: Res<WgpuRenderer>,
    // msaa: Res<Msaa>,
    world: &mut World,
) {
    if world.is_resource_changed::<Msaa>() {
        let msaa = world.resource::<Msaa>();
        let renderer = world.resource::<WgpuRenderer>();
        log::info!("updating egui render pass");
        let egui_renderer = egui_wgpu::renderer::Renderer::new(
            &renderer.device,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            None,
            msaa.samples,
        );
        // let pass = egui_wgpu::renderer::RenderPass::new(
        //     &renderer.device,
        //     wgpu::TextureFormat::Bgra8UnormSrgb,
        //     msaa.samples,
        // );
        world.insert_non_send_resource(EguiRenderer(egui_renderer));
    }
}

fn begin_frame(
    ctx: Res<EguiCtxRes>,
    mut winit_state: ResMut<EguiWinitState>,
    windows: Query<Entity, With<Window>>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    if let Ok(window) = windows.get_single() {
        let winit_window = winit_windows
            .get_window(window)
            .expect("winit window not found");
        ctx.0.begin_frame(winit_state.take_egui_input(winit_window));
    }
}

pub fn render(
    screen_descriptor: Res<EguiScreenDesciptorRes>,
    mut egui_renderer: NonSendMut<EguiRenderer>,
    mut encoder: ResMut<WgpuEncoder>,
    view: Res<WgpuView>,
    mut paint_jobs: ResMut<PaintJobs>,
    renderer: Res<WgpuRenderer>,
    egui_ctx: Res<EguiCtxRes>,
    mut state: ResMut<EguiWinitState>,
    windows: Query<Entity, With<Window>>,
    winit_windows: NonSend<WinitWindows>,
) {
    let window = if let Ok(window) = windows.get_single() {
        winit_windows
            .get_window(window)
            .expect("Failed to get primary window")
    } else {
        return;
    };

    let encoder = if let Some(encoder) = encoder.0.as_mut() {
        encoder
    } else {
        return;
    };

    // log::info!("render egui");

    let egui::FullOutput {
        shapes,
        textures_delta,
        platform_output,
        ..
    } = egui_ctx.0.end_frame();

    paint_jobs.0 = egui_ctx.0.tessellate(shapes);

    state.handle_platform_output(window, &egui_ctx.0, platform_output);

    for (id, image_delta) in textures_delta.set {
        egui_renderer
            .0
            .update_texture(&renderer.device, &renderer.queue, id, &image_delta);
    }

    egui_renderer.0.update_buffers(
        &renderer.device,
        &renderer.queue,
        encoder,
        &paint_jobs.0,
        &screen_descriptor.0,
    );

    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(view.get_color_attachment(wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: true,
        }))],
        depth_stencil_attachment: None,
        label: Some("egui main render pass"),
    });
    rpass.push_debug_group("egui_pass");

    egui_renderer
        .0
        .render(&mut rpass, &paint_jobs.0, &screen_descriptor.0);

    rpass.pop_debug_group();
}

/// Wraps bevy mouse events and convert them back to fake winit events to send to the egui winit platform support
fn handle_mouse_events(
    mut mouse_button_input_events: EventReader<MouseButtonInput>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut platform: ResMut<EguiWinitState>,
    ctx: ResMut<EguiCtxRes>,
    windows: Query<&Window>,
) {
    let window_height = if let Ok(window) = windows.get_single() {
        window.physical_height()
    } else {
        return;
    };

    for ev in cursor_moved_events.iter() {
        platform.on_event(
            &ctx.0,
            &winit::event::WindowEvent::CursorMoved {
                device_id: unsafe { winit::event::DeviceId::dummy() },
                modifiers: winit::event::ModifiersState::empty(),
                position: winit::dpi::PhysicalPosition {
                    x: ev.position.x as f64,
                    y: if ev.position.y as u32 > window_height {
                        0.0
                    } else {
                        (window_height - ev.position.y as u32) as f64
                    },
                },
            },
        );
    }

    for ev in mouse_button_input_events.iter() {
        platform.on_event(
            &ctx.0,
            &winit::event::WindowEvent::MouseInput {
                device_id: unsafe { winit::event::DeviceId::dummy() },
                modifiers: winit::event::ModifiersState::empty(),
                state: match ev.state {
                    bevy::input::ButtonState::Pressed => winit::event::ElementState::Pressed,
                    bevy::input::ButtonState::Released => winit::event::ElementState::Released,
                },
                button: match ev.button {
                    MouseButton::Left => winit::event::MouseButton::Left,
                    MouseButton::Right => winit::event::MouseButton::Right,
                    MouseButton::Middle => winit::event::MouseButton::Middle,
                    MouseButton::Other(x) => winit::event::MouseButton::Other(x),
                },
            },
        );
    }

    for ev in mouse_wheel_events.iter() {
        platform.on_event(
            &ctx.0,
            &winit::event::WindowEvent::MouseWheel {
                device_id: unsafe { winit::event::DeviceId::dummy() },
                modifiers: winit::event::ModifiersState::empty(),
                phase: winit::event::TouchPhase::Moved,
                delta: match ev.unit {
                    bevy::input::mouse::MouseScrollUnit::Line => {
                        winit::event::MouseScrollDelta::LineDelta(ev.x, ev.y)
                    }
                    bevy::input::mouse::MouseScrollUnit::Pixel => {
                        winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition {
                            x: ev.x as f64,
                            y: ev.y as f64,
                        })
                    }
                },
            },
        );
    }
}
