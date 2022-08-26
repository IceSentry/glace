use bevy::{ecs::system::Resource, prelude::*, window::WindowResized, winit::WinitWindows};
use futures_lite::future;
use winit::dpi::PhysicalSize;

use crate::{
    camera::{Camera, CameraPlugin},
    egui_plugin::{EguiRenderPhase, EguiScreenDesciptorRes},
    instances,
    renderer::{RenderPhase, WgpuRenderer},
    texture::Texture,
};

use super::{
    bind_groups::{self, mesh_view::CameraUniform},
    depth_pass::DepthPass,
    render_phase_3d::{DepthTexture, RenderPhase3d},
    wireframe::WireframePhase,
};

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
                "init_render_phase",
                SystemStage::parallel(),
            )
            .add_startup_system_to_stage("init_render_phase", init_render_phase.exclusive_system())
            .add_startup_system_to_stage(
                "init_render_phase",
                init_wireframe_phase.exclusive_system(),
            )
            .add_startup_system(init_depth_pass)
            .add_startup_system_to_stage(
                // Needs to be in PostStartup because it sets up the bind_group based on
                // what was spawned in the startup
                StartupStage::PostStartup,
                bind_groups::mesh_view::setup_mesh_view_bind_group,
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                update_render_phase::<RenderPhase3d>
                    .exclusive_system()
                    .before("render"),
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                update_render_phase::<EguiRenderPhase>
                    .exclusive_system()
                    .before("render"),
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                update_render_phase::<WireframePhase>
                    .exclusive_system()
                    .before("render"),
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                render.exclusive_system().label("render"),
            )
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

fn init_render_phase(world: &mut World) {
    // TODO look into FromWorld
    let render_phase_3d = RenderPhase3d::from_world(world);
    world.insert_resource(render_phase_3d);
}

fn init_wireframe_phase(world: &mut World) {
    let render_phase_3d = WireframePhase::from_world(world);
    world.insert_resource(render_phase_3d);
}

fn init_depth_pass(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let depth_texture = Texture::create_depth_texture(&renderer.device, &renderer.config);
    let depth_pass = DepthPass::new(&renderer, &depth_texture);
    commands.insert_resource(DepthTexture(depth_texture));
    commands.insert_resource(depth_pass);
}

fn render(world: &World, renderer: Res<WgpuRenderer>) {
    if let Err(e) = renderer.render(world) {
        log::error!("{e:?}")
    };
}

fn update_render_phase<T: RenderPhase + Resource>(world: &mut World) {
    world.resource_scope(|world, mut phase: Mut<T>| {
        phase.update(world);
    });
}

#[allow(clippy::too_many_arguments)]
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
