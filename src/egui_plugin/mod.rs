use self::custom_egui_winit::EguiWinitState;
use crate::renderer::{RenderPhase, WgpuRenderer};
use bevy::{
    app::AppExit,
    ecs::system::SystemState,
    input::mouse::{MouseButtonInput, MouseWheel},
    prelude::*,
    window::WindowCloseRequested,
    winit::WinitWindows,
};

mod custom_egui_winit;

#[derive(Resource)]
pub struct EguiCtxRes(pub egui::Context);

#[derive(Resource)]
pub struct EguiScreenDesciptorRes(pub egui_wgpu::renderer::ScreenDescriptor);

#[derive(Resource)]
struct EguiRenderPassRes(egui_wgpu::renderer::RenderPass);

#[derive(Resource)]
#[allow(clippy::type_complexity)]
pub struct EguiRenderPhase<'w> {
    state: SystemState<(
        Res<'w, WgpuRenderer>,
        Res<'w, EguiScreenDesciptorRes>,
        NonSend<'w, EguiCtxRes>,
        NonSendMut<'w, EguiRenderPassRes>,
        ResMut<'w, EguiWinitState>,
        Res<'w, Windows>,
        NonSend<'w, WinitWindows>,
    )>,
    paint_jobs: Vec<egui::ClippedPrimitive>,
}

pub struct EguiPlugin;
impl Plugin for EguiPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup)
            .add_startup_system(setup_render_pass.exclusive_system())
            .add_startup_system(setup_render_phase.exclusive_system())
            .add_system_to_stage(CoreStage::PreUpdate, begin_frame)
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
}

fn setup_render_pass(world: &mut World) {
    let renderer = world.resource::<WgpuRenderer>();
    let pass = egui_wgpu::renderer::RenderPass::new(
        &renderer.device,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        1,
    );
    world.insert_non_send_resource(EguiRenderPassRes(pass));
}

fn setup_render_phase(world: &mut World) {
    let initial_state = SystemState::new(world);
    world.insert_resource(EguiRenderPhase {
        state: initial_state,
        paint_jobs: Vec::new(),
    });
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

impl<'w> RenderPhase for EguiRenderPhase<'w> {
    fn update(&mut self, world: &mut World) {
        // TODO look if WorldQuery could help simplify this a bit

        let (
            renderer,
            screen_descriptor,
            egui_ctx,
            mut render_pass,
            mut platform,
            windows,
            winit_windows,
        ) = self.state.get_mut(world);

        let egui::FullOutput {
            shapes,
            textures_delta,
            platform_output,
            ..
        } = egui_ctx.0.end_frame();

        self.paint_jobs = egui_ctx.0.tessellate(shapes);

        let window = if let Some(window) = windows.get_primary() {
            winit_windows
                .get_window(window.id())
                .expect("Failed to get primary window")
        } else {
            return;
        };

        platform.handle_platform_output(window, &egui_ctx.0, platform_output);

        for (id, image_delta) in textures_delta.set {
            render_pass
                .0
                .update_texture(&renderer.device, &renderer.queue, id, &image_delta);
        }

        render_pass.0.update_buffers(
            &renderer.device,
            &renderer.queue,
            &self.paint_jobs,
            &screen_descriptor.0,
        );
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let screen_descriptor = world.resource::<EguiScreenDesciptorRes>();
        let render_pass = world.non_send_resource::<EguiRenderPassRes>();

        render_pass
            .0
            .execute(encoder, view, &self.paint_jobs, &screen_descriptor.0, None)
    }
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
