use bevy::{prelude::*, window::WindowResized, winit::WinitWindows};
use futures_lite::future;
use wgpu::{CommandEncoder, SurfaceTexture, TextureView};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    camera::{Camera, CameraPlugin},
    egui_plugin::EguiScreenDesciptorRes,
    instances,
    texture::Texture,
};

use self::{
    base_3d::RenderPhase3dPlugin, bind_groups::mesh_view::CameraUniform, wireframe::WireframePlugin,
};

pub mod base_3d;
pub mod bind_groups;
pub mod wireframe;

#[derive(Resource)]
pub struct DepthTexture(pub Texture);

#[derive(Default, Resource)]
pub struct GlaceClearColor(pub Color);

pub const SAMPLE_COUNT: u32 = 4;

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
            .add_plugin(WireframePlugin)
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
pub struct WgpuView {
    pub view: TextureView,
    pub sampled_view: Option<TextureView>,
}

impl WgpuView {
    pub fn get_color_attachment(
        &self,
        ops: wgpu::Operations<wgpu::Color>,
    ) -> wgpu::RenderPassColorAttachment {
        wgpu::RenderPassColorAttachment {
            view: self.sampled_view.as_ref().unwrap_or(&self.view),
            resolve_target: self.sampled_view.as_ref().map(|_| &self.view),
            ops,
        }
    }
}

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
    commands.insert_resource(WgpuView {
        view,
        sampled_view: if SAMPLE_COUNT > 1 {
            Some(create_multisampled_framebuffer(
                &renderer.device,
                &renderer.config,
                SAMPLE_COUNT,
            ))
        } else {
            None
        },
    });
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

        // Should probably be done in EguiPlugin
        screen_descriptor.0.size_in_pixels = [width as u32, height as u32];
    }
}

#[derive(Resource)]
pub struct WgpuRenderer {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
}

impl WgpuRenderer {
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to request adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::POLYGON_MODE_LINE,
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .expect("Failed to request device");

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_supported_formats(&adapter)[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
        };
        surface.configure(&device, &config);

        Self {
            surface,
            device,
            queue,
            config,
            size,
        }
    }

    pub fn create_render_pipeline(
        &self,
        label: &str,
        shader: &str,
        pipeline_layout: &wgpu::PipelineLayout,
        vertex_layouts: &[wgpu::VertexBufferLayout],
        depth_stencil: Option<wgpu::DepthStencilState>,
        blend: wgpu::BlendState,
        sample_count: u32,
    ) -> wgpu::RenderPipeline {
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&format!("{label} Shader")),
                source: wgpu::ShaderSource::Wgsl(shader.into()),
            });
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vertex",
                    buffers: vertex_layouts,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fragment",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..default()
                },
                depth_stencil,
                multisample: wgpu::MultisampleState {
                    count: sample_count,
                    ..default()
                },
                multiview: None,
            })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        } else {
            log::info!("window has been minimized")
        }
    }
}

pub fn create_multisampled_framebuffer(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    sample_count: u32,
) -> wgpu::TextureView {
    let multisampled_texture_extent = wgpu::Extent3d {
        width: config.width,
        height: config.height,
        depth_or_array_layers: 1,
    };
    let multisampled_frame_descriptor = &wgpu::TextureDescriptor {
        size: multisampled_texture_extent,
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        label: None,
    };

    device
        .create_texture(multisampled_frame_descriptor)
        .create_view(&wgpu::TextureViewDescriptor::default())
}
