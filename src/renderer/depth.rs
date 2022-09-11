use bevy::{
    prelude::*,
    render::render_resource::{encase, ShaderType},
};
use wgpu::util::DeviceExt;

use super::{
    DepthTexture, RenderLabel, RendererStage, WgpuEncoder, WgpuRenderer, WgpuView, SAMPLE_COUNT,
};
use crate::{mesh::Vertex, model::ModelMesh, shapes::quad::FullscreenQuad, texture::Texture};

const DEFAULT_NEAR: f32 = 0.1;
const DEFAULT_FAR: f32 = 1000.0;

#[derive(Default, Resource)]
pub struct DepthPassSettings {
    pub show_depth_buffer: bool,
}

pub struct DepthPassPlugin;
impl Plugin for DepthPassPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app
            // .add_startup_system_to_stage(RendererStage::Init, setup)
            // .add_system_to_stage(
            //     RendererStage::Render,
            //     render
            //         .label(RenderLabel::Depth)
            //         .after(RenderLabel::Wireframe),
            // )
            .init_resource::<DepthPassSettings>();
    }
}

fn setup(mut commands: Commands, renderer: Res<WgpuRenderer>, depth_texture: Res<DepthTexture>) {
    commands.insert_resource(DepthPass::new(&renderer, &depth_texture.0));
}

fn render(
    depth_pass: Res<DepthPass>,
    mut encoder: ResMut<WgpuEncoder>,
    view: Res<WgpuView>,
    settings: Res<DepthPassSettings>,
) {
    if !settings.show_depth_buffer {
        return;
    }

    let encoder = if let Some(encoder) = encoder.0.as_mut() {
        encoder
    } else {
        return;
    };

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Depth Render Pass"),
        color_attachments: &[Some(view.get_color_attachment(wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: SAMPLE_COUNT == 1,
        }))],
        depth_stencil_attachment: None,
    });
    render_pass.set_pipeline(&depth_pass.render_pipeline);
    render_pass.set_bind_group(0, &depth_pass.bind_group, &[]);
    render_pass.set_vertex_buffer(0, depth_pass.mesh.vertex_buffer.slice(..));
    render_pass.set_index_buffer(
        depth_pass.mesh.index_buffer.slice(..),
        wgpu::IndexFormat::Uint32,
    );
    render_pass.draw_indexed(0..depth_pass.mesh.num_elements, 0, 0..1);
}

#[derive(ShaderType)]
struct DepthPassMaterial {
    near: f32,
    far: f32,
}

#[derive(Resource)]
pub struct DepthPass {
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    mesh: ModelMesh,
}

impl DepthPass {
    pub fn new(renderer: &WgpuRenderer, texture: &Texture) -> Self {
        let layout = DepthPass::bind_group_layout(&renderer.device);
        let bind_group = DepthPass::bind_group(
            &renderer.device,
            &layout,
            texture,
            DepthPassMaterial {
                near: DEFAULT_NEAR,
                far: DEFAULT_FAR,
            },
        );

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Depth Pass Pipeline Layout"),
                    bind_group_layouts: &[&DepthPass::bind_group_layout(&renderer.device)],
                    push_constant_ranges: &[],
                });

        let render_pipeline = renderer.create_render_pipeline(
            "Depth Pass Render Pipeline",
            include_str!("shaders/depth.wgsl"),
            &pipeline_layout,
            &[Vertex::layout()],
            None,
            wgpu::BlendState::REPLACE,
            SAMPLE_COUNT,
        );

        Self {
            layout,
            bind_group,
            render_pipeline,
            mesh: FullscreenQuad.mesh(&renderer.device),
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, texture: &Texture) {
        self.bind_group = DepthPass::bind_group(
            device,
            &self.layout,
            texture,
            DepthPassMaterial {
                near: DEFAULT_NEAR,
                far: DEFAULT_FAR,
            },
        );
    }

    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Depth Pass Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &Texture,
        material: DepthPassMaterial,
    ) -> wgpu::BindGroup {
        let byte_buffer = [0u8; std::mem::size_of::<f32>() * 2];
        let mut buffer = encase::UniformBuffer::new(byte_buffer);
        buffer.write(&material).unwrap();

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            contents: buffer.as_ref(),
            label: None,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("depth_pass.bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        })
    }
}
