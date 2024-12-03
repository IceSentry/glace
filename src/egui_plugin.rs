use bevy::{
    app::{prelude::*, AppExit},
    ecs::prelude::*,
    prelude::{Deref, DerefMut},
    window::{prelude::*, PrimaryWindow, WindowCloseRequested},
    winit::{RawWinitWindowEvent, WinitWindows},
};
use wgpu::{rwh::HasDisplayHandle, CommandEncoder, TextureView};

use crate::{setup_renderer, Device, Queue, Surface};

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
            .add_systems(Update, (handle_winit_events, on_exit));
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

fn handle_winit_events(
    mut winit_events: EventReader<RawWinitWindowEvent>,
    windows: Query<Entity, With<PrimaryWindow>>,
    winit_windows: NonSend<WinitWindows>,
    mut egui_winit_state: ResMut<EguiWinitState>,
) {
    let window = if let Ok(window) = windows.get_single() {
        winit_windows
            .get_window(window)
            .expect("Failed to get primary window")
    } else {
        return;
    };
    for ev in winit_events.read() {
        let _ = egui_winit_state.on_window_event(window, &ev.event);
    }
}
