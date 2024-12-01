use bevy::{
    app::{prelude::*, AppExit},
    ecs::prelude::*,
    input::{
        mouse::{MouseButtonInput, MouseWheel},
        prelude::*,
    },
    prelude::{Deref, DerefMut},
    window::{
        prelude::*, PrimaryWindow, WindowCloseRequested, WindowResized, WindowScaleFactorChanged,
    },
    winit::WinitWindows,
};
use wgpu::{rwh::HasDisplayHandle, CommandEncoder, TextureView};
use winit::dpi::PhysicalSize;

//use self::custom_egui_winit::EguiWinitState;
use crate::{setup_renderer, Device, Queue, Surface};

//pub mod custom_egui_winit;

#[derive(Resource, Deref, DerefMut)]
pub struct EguiCtxRes(pub egui::Context);

#[derive(Resource)]
pub struct EguiScreenDesciptorRes(pub egui_wgpu::ScreenDescriptor);

#[derive(Resource)]
pub struct EguiPaintJobs(Vec<egui::ClippedPrimitive>);

#[derive(Resource, Deref, DerefMut)]
pub struct EguiWinitState(pub egui_winit::State);

#[derive(Resource, Deref, DerefMut)]
pub struct EguiRenderer(egui_wgpu::Renderer);

pub struct EguiPlugin;
impl Plugin for EguiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup, setup_render_pass).after(setup_renderer))
            .add_systems(PreUpdate, begin_frame)
            // .add_system(update_render_pass)
            // .add_system(render)
            .add_systems(Update, (handle_mouse_events, handle_window_events, on_exit));
    }
}

fn on_exit(
    exit: EventReader<AppExit>,
    window_close: EventReader<WindowCloseRequested>,
    _egui_ctx: Res<EguiCtxRes>,
) {
    if !exit.is_empty() || !window_close.is_empty() {
        //egui_ctx.0.memory(|mem| {
        //    std::fs::write(
        //        "egui.ron",
        //        ron::ser::to_string_pretty(&mem, ron::ser::PrettyConfig::new())
        //            .expect("failed to serialize egui memory"),
        //    )
        //    .expect("Failed to write egui memory");
        //})
    }
}

fn setup(
    mut commands: Commands,
    windows_entity: Query<Entity, With<PrimaryWindow>>,
    winit_windows: NonSend<WinitWindows>,
) {
    let ctx = egui::Context::default();
    //if let Ok(mem) = std::fs::read_to_string("egui.ron") {
    //    let mem: egui::Memory = ron::de::from_str(&mem).expect("Failed to deserialize egui.ron");
    //    ctx.memory_mut(|memory| {
    //        memory.clone_from(&mem);
    //    })
    //}
    if let Ok(window) = windows_entity.get_single() {
        let winit_window = winit_windows
            .get_window(window)
            .expect("winit window not found");
        commands.insert_resource(EguiWinitState(egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            &winit_window
                .display_handle()
                .expect("Failed to get display handle"),
            None,
            None,
            None,
        )));
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [
                winit_window.inner_size().width,
                winit_window.inner_size().height,
            ],
            pixels_per_point: winit_window.scale_factor() as f32,
        };
        commands.insert_resource(EguiScreenDesciptorRes(screen_descriptor));
    }
    commands.insert_resource(EguiCtxRes(ctx));
    commands.insert_resource(EguiPaintJobs(vec![]));
}

fn setup_render_pass(world: &mut World) {
    let device = world.resource::<Device>();
    // We render egui directly to the swapchain so we use the surface format
    let format = world
        .resource::<Surface>()
        .get_current_texture()
        .expect("Failed to get surface texture while initializing egui")
        .texture
        .format();
    let egui_renderer = egui_wgpu::Renderer::new(&device.0, format, None, 1, false);
    world.insert_non_send_resource(EguiRenderer(egui_renderer));
}

fn begin_frame(
    egui_ctx: Res<EguiCtxRes>,
    mut winit_state: ResMut<EguiWinitState>,
    windows: Query<Entity, With<Window>>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    if let Ok(window) = windows.get_single() {
        let winit_window = winit_windows
            .get_window(window)
            .expect("winit window not found");
        egui_ctx.begin_pass(winit_state.take_egui_input(winit_window));
    }
}

pub fn egui_render_pass(
    window: &winit::window::Window,
    egui_renderer: &mut EguiRenderer,
    paint_jobs: &mut EguiPaintJobs,
    egui_ctx: &EguiCtxRes,
    state: &mut EguiWinitState,
    screen_descriptor: &EguiScreenDesciptorRes,
    device: &Device,
    queue: &Queue,
    encoder: &mut CommandEncoder,
    view: &TextureView,
) {
    let egui::FullOutput {
        shapes,
        textures_delta,
        platform_output,
        ..
    } = egui_ctx.end_pass();

    state.handle_platform_output(window, platform_output);
    if window.inner_size().width < screen_descriptor.0.size_in_pixels[0] {
        //warn!("egui screen desc too big");
        return;
    }
    paint_jobs.0 = egui_ctx.tessellate(shapes, window.scale_factor() as f32);

    for (id, image_delta) in textures_delta.set {
        egui_renderer.update_texture(device, queue, id, &image_delta);
    }

    egui_renderer.update_buffers(device, queue, encoder, &paint_jobs.0, &screen_descriptor.0);

    {
        let mut rpass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                label: Some("egui main render pass"),
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        rpass.push_debug_group("egui_pass");

        egui_renderer.render(&mut rpass, &paint_jobs.0, &screen_descriptor.0);

        rpass.pop_debug_group();
    }
}

fn handle_window_events(
    mut egui_winit_state: ResMut<EguiWinitState>,
    windows: Query<Entity, With<PrimaryWindow>>,
    winit_windows: NonSend<WinitWindows>,
    mut screen_descriptor: ResMut<EguiScreenDesciptorRes>,
    mut scale_factor_changed_event: EventReader<WindowScaleFactorChanged>,
    mut window_resized_event: EventReader<WindowResized>,
) {
    let window = if let Ok(window) = windows.get_single() {
        winit_windows
            .get_window(window)
            .expect("Failed to get primary window")
    } else {
        return;
    };
    for ev in scale_factor_changed_event.read() {
        screen_descriptor.0.pixels_per_point = ev.scale_factor as f32;
        let _ = egui_winit_state.on_window_event(
            window,
            &winit::event::WindowEvent::ScaleFactorChanged {
                scale_factor: ev.scale_factor,
                inner_size_writer: unsafe {
                    std::mem::transmute::<[u8; 8], winit::event::InnerSizeWriter>(
                        [0u8; std::mem::size_of::<winit::event::InnerSizeWriter>()],
                    )
                },
            },
        );
    }
    for ev in window_resized_event.read() {
        screen_descriptor.0.size_in_pixels[0] = ev.width as u32;
        screen_descriptor.0.size_in_pixels[1] = ev.height as u32;
        let _ = egui_winit_state.on_window_event(
            window,
            &winit::event::WindowEvent::Resized(PhysicalSize::new(
                ev.width as u32,
                ev.height as u32,
            )),
        );
    }
}

/// Wraps bevy mouse events and convert them back to fake winit events to send to the egui winit platform support
fn handle_mouse_events(
    mut mouse_button_input_events: EventReader<MouseButtonInput>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut platform: ResMut<EguiWinitState>,
    windows: Query<Entity, With<PrimaryWindow>>,
    winit_windows: NonSend<WinitWindows>,
) {
    let window = if let Ok(window) = windows.get_single() {
        winit_windows
            .get_window(window)
            .expect("Failed to get primary window")
    } else {
        return;
    };
    let window_height = window.inner_size().height;

    for ev in cursor_moved_events.read() {
        let _ = platform.on_window_event(
            window,
            &winit::event::WindowEvent::CursorMoved {
                device_id: winit::event::DeviceId::dummy(),
                position: winit::dpi::PhysicalPosition {
                    x: ev.position.x as f64,
                    y: if ev.position.y as u32 > window_height {
                        0.0
                    } else {
                        (ev.position.y as u32) as f64
                    },
                },
            },
        );
    }

    for ev in mouse_button_input_events.read() {
        let _ = platform.on_window_event(
            window,
            &winit::event::WindowEvent::MouseInput {
                device_id: winit::event::DeviceId::dummy(),
                state: match ev.state {
                    bevy::input::ButtonState::Pressed => winit::event::ElementState::Pressed,
                    bevy::input::ButtonState::Released => winit::event::ElementState::Released,
                },
                button: match ev.button {
                    MouseButton::Left => winit::event::MouseButton::Left,
                    MouseButton::Right => winit::event::MouseButton::Right,
                    MouseButton::Middle => winit::event::MouseButton::Middle,
                    MouseButton::Back => winit::event::MouseButton::Back,
                    MouseButton::Forward => winit::event::MouseButton::Forward,
                    MouseButton::Other(x) => winit::event::MouseButton::Other(x),
                },
            },
        );
    }

    for ev in mouse_wheel_events.read() {
        let _ = platform.on_window_event(
            window,
            &winit::event::WindowEvent::MouseWheel {
                device_id: winit::event::DeviceId::dummy(),
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
