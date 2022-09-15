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
use crate::renderer::{Msaa, RenderLabel, RendererStage, WgpuEncoder, WgpuRenderer, WgpuView};

mod custom_egui_winit;

#[derive(Resource)]
pub struct EguiCtxRes(pub egui::Context);

#[derive(Resource)]
pub struct EguiScreenDesciptorRes(pub egui_wgpu::renderer::ScreenDescriptor);

#[derive(Resource)]
struct EguiRenderPassRes(egui_wgpu::renderer::RenderPass);

#[derive(Resource)]
struct PaintJobs(Vec<egui::ClippedPrimitive>);

pub struct EguiPlugin;
impl Plugin for EguiPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup)
            .add_startup_system(setup_render_pass.exclusive_system())
            .add_system_to_stage(CoreStage::PreUpdate, begin_frame)
            .add_system_to_stage(
                RendererStage::Render,
                render
                    .label(RenderLabel::Egui)
                    .after(RenderLabel::Wireframe),
            )
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
        let mem = egui_ctx.0.memory().clone();
        std::fs::write(
            "egui.ron",
            ron::ser::to_string_pretty(&mem, ron::ser::PrettyConfig::new())
                .expect("failed to serialize egui memory"),
        )
        .expect("Failed to write egui memory");
    }
}

fn setup(mut commands: Commands, windows: Res<Windows>) {
    let window = windows.primary();
    let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
        size_in_pixels: [window.width() as u32, window.height() as u32],
        pixels_per_point: window.scale_factor() as f32,
    };
    commands.insert_resource(EguiScreenDesciptorRes(screen_descriptor));
    commands.insert_resource(EguiWinitState::new());

    let ctx = egui::Context::default();
    if let Ok(mem) = std::fs::read_to_string("egui.ron") {
        let mem: egui::Memory = ron::de::from_str(&mem).expect("Failed to deserialize egui.ron");
        ctx.memory().clone_from(&mem);
    }

    commands.insert_resource(EguiCtxRes(ctx));
    commands.insert_resource(PaintJobs(vec![]));
}

fn setup_render_pass(world: &mut World) {
    let msaa = world.resource::<Msaa>();
    let renderer = world.resource::<WgpuRenderer>();
    let pass = egui_wgpu::renderer::RenderPass::new(
        &renderer.device,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        msaa.samples,
    );
    world.insert_non_send_resource(EguiRenderPassRes(pass));
}

fn begin_frame(
    ctx: Res<EguiCtxRes>,
    mut winit_state: ResMut<EguiWinitState>,
    windows: Res<Windows>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    if let Some(window) = windows.get_primary() {
        let winit_window = winit_windows
            .get_window(window.id())
            .expect("winit window not found");
        ctx.0.begin_frame(winit_state.take_egui_input(winit_window));
    }
}

fn render(
    screen_descriptor: Res<EguiScreenDesciptorRes>,
    mut render_pass: NonSendMut<EguiRenderPassRes>,
    mut encoder: ResMut<WgpuEncoder>,
    view: Res<WgpuView>,
    mut paint_jobs: ResMut<PaintJobs>,
    renderer: Res<WgpuRenderer>,
    egui_ctx: NonSend<EguiCtxRes>,
    mut state: ResMut<EguiWinitState>,
    windows: Res<Windows>,
    winit_windows: NonSend<WinitWindows>,
) {
    let window = if let Some(window) = windows.get_primary() {
        winit_windows
            .get_window(window.id())
            .expect("Failed to get primary window")
    } else {
        return;
    };

    let encoder = if let Some(encoder) = encoder.0.as_mut() {
        encoder
    } else {
        return;
    };

    let egui::FullOutput {
        shapes,
        textures_delta,
        platform_output,
        ..
    } = egui_ctx.0.end_frame();

    paint_jobs.0 = egui_ctx.0.tessellate(shapes);

    state.handle_platform_output(window, &egui_ctx.0, platform_output);

    for (id, image_delta) in textures_delta.set {
        render_pass
            .0
            .update_texture(&renderer.device, &renderer.queue, id, &image_delta);
    }

    render_pass.0.update_buffers(
        &renderer.device,
        &renderer.queue,
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

    render_pass
        .0
        .execute_with_renderpass(&mut rpass, &paint_jobs.0, &screen_descriptor.0);

    rpass.pop_debug_group();
}

/// Wraps bevy mouse events and convert them back to fake winit events to send to the egui winit platform support
fn handle_mouse_events(
    mut mouse_button_input_events: EventReader<MouseButtonInput>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut platform: ResMut<EguiWinitState>,
    ctx: ResMut<EguiCtxRes>,
    windows: Res<Windows>,
) {
    let window_height = if let Some(window) = windows.get_primary() {
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
