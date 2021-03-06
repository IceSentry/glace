use bevy::prelude::*;
use wgpu::CommandEncoder;
use winit::window::Window;

use crate::egui_plugin::EguiRenderPhase;

use self::{render_phase_3d::RenderPhase3d, wireframe::WireframePhase};

pub mod bind_groups;
pub mod depth_pass;
pub mod plugin;
pub mod render_phase_3d;
pub mod wireframe;

// NOTE: Is this trait necessary?
pub trait RenderPhase {
    fn update(&mut self, world: &mut World);
    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut CommandEncoder);
}

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
            format: surface.get_preferred_format(&adapter).unwrap(),
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
    ) -> wgpu::RenderPipeline {
        let shader = self
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
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
                    targets: &[wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..default()
                },
                depth_stencil,
                multisample: wgpu::MultisampleState { ..default() },
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

    pub fn render(&self, world: &World) -> anyhow::Result<()> {
        if world
            .get_resource::<Windows>()
            .and_then(|w| w.get_primary())
            .is_none()
        {
            return Ok(());
        }

        let output = match self.surface.get_current_texture() {
            Ok(swap_chain_frame) => swap_chain_frame,
            Err(wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                self.surface
                    .get_current_texture()
                    .expect("Failed to reconfigure surface")
            }
            err => err?,
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let phase = world.resource::<RenderPhase3d>();
        phase.render(world, &view, &mut encoder);

        let phase = world.resource::<WireframePhase>();
        phase.render(world, &view, &mut encoder);

        let phase = world.get_non_send_resource::<EguiRenderPhase>();
        if let Some(phase) = phase {
            phase.render(world, &view, &mut encoder);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
