use std::borrow::Cow;

use bevy::prelude::*;

use crate::{
    instances::{InstanceBuffer, Instances},
    light::Light,
    mesh::Vertex,
    model::Model,
    texture::Texture,
    transform::TransformRaw,
};

use super::{
    bind_groups::mesh_view::{MeshViewBindGroup, MeshViewBindGroupLayout},
    render_phase_3d::DepthTexture,
    RenderPhase, WgpuRenderer,
};

const SAMPLE_COUNT: usize = 1;
const USE_DEPTH: bool = true;

#[derive(Component)]
pub struct Wireframe;

pub struct WireframePhase {
    pub render_pipeline: wgpu::RenderPipeline,
    pub multisampled_framebuffer: wgpu::TextureView,
    pub model_query: QueryState<
        (
            &'static Model,
            &'static InstanceBuffer,
            Option<&'static Instances>,
        ),
        (Without<Light>, With<Wireframe>),
    >,
}

impl RenderPhase for WireframePhase {
    fn update<'w>(&'w mut self, world: &'w mut World) {
        self.model_query.update_archetypes(world);
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mesh_view_bind_group = world.resource::<MeshViewBindGroup>();
        let depth_texture = world.resource::<DepthTexture>();

        let rpass_color_attachment = if SAMPLE_COUNT == 1 {
            wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            }
        } else {
            wgpu::RenderPassColorAttachment {
                view: &self.multisampled_framebuffer,
                resolve_target: Some(view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    // Storing pre-resolve MSAA data is unnecessary if it isn't used later.
                    // On tile-based GPU, avoid store can reduce your app's memory footprint.
                    store: false,
                },
            }
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[rpass_color_attachment],
            depth_stencil_attachment: if USE_DEPTH {
                Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_texture.0.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    }),
                    stencil_ops: None,
                })
            } else {
                None
            },
        });

        render_pass.set_pipeline(&self.render_pipeline);

        for (model, instance_buffer, instances) in self.model_query.iter_manual(world) {
            render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));
            for mesh in model.meshes.iter() {
                // mesh.vertex_buffer
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.set_bind_group(0, &mesh_view_bind_group.0, &[]);
                render_pass.draw_indexed(
                    0..mesh.num_elements,
                    0,
                    0..instances.map(|i| i.0.len() as u32).unwrap_or(1),
                );
            }
        }
    }
}

impl WireframePhase {
    pub fn from_world(world: &mut World) -> Self {
        let renderer = world.resource::<WgpuRenderer>();
        let mesh_view_layout = world.resource::<MeshViewBindGroupLayout>();

        let shader = renderer
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "shaders/wireframe.wgsl"
                ))),
            });

        let pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&mesh_view_layout.0],
                    push_constant_ranges: &[],
                });

        let multisampled_framebuffer = create_multisampled_framebuffer(
            &renderer.device,
            &renderer.config,
            SAMPLE_COUNT as u32,
        );

        let pipeline = renderer
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vertex",
                    buffers: &[Vertex::layout(), TransformRaw::layout()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fragment",
                    targets: &[wgpu::ColorTargetState {
                        format: renderer.config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    polygon_mode: wgpu::PolygonMode::Line,
                    ..default()
                },
                depth_stencil: if USE_DEPTH {
                    Some(wgpu::DepthStencilState {
                        format: Texture::DEPTH_FORMAT,
                        depth_write_enabled: false,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState {
                            slope_scale: -1.0,
                            ..default()
                        },
                    })
                } else {
                    None
                },
                multisample: wgpu::MultisampleState {
                    count: SAMPLE_COUNT as u32,
                    ..default()
                },
                multiview: None,
            });

        Self {
            render_pipeline: pipeline,
            multisampled_framebuffer,
            model_query: world.query_filtered(),
        }
    }
}

fn create_multisampled_framebuffer(
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
