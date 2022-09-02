use bevy::{prelude::*, window::WindowResized, winit::WinitWindows};
use futures_lite::future;
use wgpu::{CommandEncoder, SurfaceTexture, TextureView};
use winit::dpi::PhysicalSize;

use super::{
    base_3d::RenderPhase3dPlugin,
    bind_groups::{self, mesh_view::CameraUniform},
    depth::{DepthPass, DepthPassPlugin},
    DepthTexture, WgpuRenderer,
};
use crate::{
    camera::{Camera, CameraPlugin},
    egui_plugin::EguiScreenDesciptorRes,
    instances,
    texture::Texture,
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, StageLabel)]
pub enum RendererStage {
    StartRender,
    Render,
    EndRender,
    Init,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
pub enum RenderLabel {
    Base3d,
    Wireframe,
    Depth,
    Egui,
}

pub struct WgpuRendererPlugin;
impl Plugin for WgpuRendererPlugin {
    fn build(&self, app: &mut App) {
        app
            // Add the camera plugin here because it's required for the renderer to work
            .add_plugin(CameraPlugin)
            // This startup system needs to be run before any startup that needs the WgpuRenderer
            .add_startup_system_to_stage(StartupStage::PreStartup, init_renderer)
            .add_startup_stage_after(
                StartupStage::PostStartup,
                RendererStage::Init,
                SystemStage::parallel(),
            )
            .add_startup_system(init_depth_texture)
            .add_startup_system_to_stage(
                // Needs to be in PostStartup because it sets up the bind_group based on
                // what was spawned in the startup
                StartupStage::PostStartup,
                bind_groups::mesh_view::setup_mesh_view_bind_group,
            )
            .add_stage_after(
                CoreStage::PostUpdate,
                RendererStage::StartRender,
                SystemStage::parallel(),
            )
            .add_stage_after(
                RendererStage::StartRender,
                RendererStage::Render,
                SystemStage::parallel(),
            )
            .add_stage_after(
                RendererStage::Render,
                RendererStage::EndRender,
                SystemStage::parallel(),
            )
            .add_plugin(RenderPhase3dPlugin)
            .add_plugin(DepthPassPlugin)
            .add_system_to_stage(RendererStage::StartRender, start_render)
            .add_system_to_stage(RendererStage::EndRender, end_render)
            .add_system(bind_groups::mesh_view::update_light_buffer)
            .add_system(bind_groups::mesh_view::update_camera_buffer)
            .add_system(bind_groups::material::update_material_buffer)
            .add_system(bind_groups::material::create_material_uniform)
            .add_system(instances::update_instance_buffer)
            .add_system(instances::create_instance_buffer)
            .add_system(resize);
    }
}

fn init_renderer(
    mut commands: Commands,
    windows: Res<Windows>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    let winit_window = windows
        .get_primary()
        .and_then(|window| winit_windows.get_window(window.id()))
        .expect("Failed to get window");

    let renderer = future::block_on(WgpuRenderer::new(winit_window));
    commands.insert_resource(renderer);
}

fn init_depth_texture(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let depth_texture = Texture::create_depth_texture(&renderer.device, &renderer.config);
    commands.insert_resource(DepthTexture(depth_texture));
}

#[derive(Resource)]
pub struct WgpuSurfaceTexture(pub Option<SurfaceTexture>);

#[derive(Resource)]
pub struct WgpuView(pub TextureView);

#[derive(Resource)]
pub struct WgpuEncoder(pub Option<CommandEncoder>);

fn start_render(mut commands: Commands, renderer: Res<WgpuRenderer>, windows: Res<Windows>) {
    if windows.get_primary().is_none() {
        return;
    }

    let output = match renderer.surface.get_current_texture() {
        Ok(swap_chain_frame) => swap_chain_frame,
        Err(wgpu::SurfaceError::Outdated) => {
            renderer
                .surface
                .configure(&renderer.device, &renderer.config);
            renderer
                .surface
                .get_current_texture()
                .expect("Failed to reconfigure surface")
        }
        err => {
            log::error!("failed  to get surface texture. {err:?}");
            return;
        }
    };

    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let encoder = renderer
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

    commands.insert_resource(WgpuSurfaceTexture(Some(output)));
    commands.insert_resource(WgpuView(view));
    commands.insert_resource(WgpuEncoder(Some(encoder)));
}

fn end_render(
    renderer: Res<WgpuRenderer>,
    windows: Res<Windows>,
    mut encoder: ResMut<WgpuEncoder>,
    mut output: ResMut<WgpuSurfaceTexture>,
) {
    if windows.get_primary().is_none() {
        return;
    }
    if let Some(encoder) = encoder.0.take() {
        renderer.queue.submit(std::iter::once(encoder.finish()));
        output.0.take().unwrap().present();
    }
}

fn resize(
    mut renderer: ResMut<WgpuRenderer>,
    mut events: EventReader<WindowResized>,
    windows: Res<Windows>,
    mut depth_pass: ResMut<DepthPass>,
    mut depth_texture: ResMut<DepthTexture>,
    mut camera_uniform: ResMut<CameraUniform>,
    mut camera: ResMut<Camera>,
    mut screen_descriptor: ResMut<EguiScreenDesciptorRes>,
) {
    for event in events.iter() {
        let window = windows.get(event.id).expect("window not found");
        let width = window.physical_width();
        let height = window.physical_height();

        // Should probably be done in CameraPlugin
        camera.projection.resize(width, height);
        camera_uniform.update_view_proj(&camera);

        renderer.resize(PhysicalSize { width, height });

        depth_texture.0 = Texture::create_depth_texture(&renderer.device, &renderer.config);
        depth_pass.resize(&renderer.device, &depth_texture.0);

        // Should probably be done in EguiPlugin
        screen_descriptor.0.size_in_pixels = [width as u32, height as u32];
    }
}
