use crate::{renderer::WgpuRenderer, texture::Texture};
use bevy::{
    prelude::Resource,
    render::render_resource::{encase, ShaderType},
};
use wgpu::util::DeviceExt;

const DEFAULT_NEAR: f32 = 0.1;
const DEFAULT_FAR: f32 = 1000.0;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    pub fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

// This is just a quad
const DEPTH_VERTICES: &[Vertex] = &[
    Vertex {
        position: [-1.0, -1.0, 0.0],
        uv: [0.0, 1.0],
    },
    Vertex {
        position: [1.0, -1.0, 0.0],
        uv: [1.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0, 0.0],
        uv: [1.0, 0.0],
    },
    Vertex {
        position: [-1.0, 1.0, 0.0],
        uv: [0.0, 0.0],
    },
];

const DEPTH_INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

#[derive(ShaderType)]
struct DepthPassMaterial {
    near: f32,
    far: f32,
}

#[derive(Resource)]
pub struct DepthPass {
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_depth_indices: u32,
    render_pipeline: wgpu::RenderPipeline,
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

        let vertex_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Depth Pass VB"),
                contents: bytemuck::cast_slice(DEPTH_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = renderer
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Depth Pass IB"),
                contents: bytemuck::cast_slice(DEPTH_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

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
        );

        Self {
            layout,
            bind_group,
            vertex_buffer,
            index_buffer,
            num_depth_indices: DEPTH_INDICES.len() as u32,
            render_pipeline,
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

    pub fn render(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Depth Visual Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.num_depth_indices, 0, 0..1);
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
