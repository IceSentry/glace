use bevy::prelude::*;
use std::borrow::Cow;

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
    plugin::{RenderLabel, RendererStage, WgpuEncoder, WgpuView},
    DepthTexture, WgpuRenderer,
};

const SAMPLE_COUNT: usize = 1;
const USE_DEPTH: bool = true;

#[derive(Component)]
pub struct Wireframe;

#[derive(Resource)]
pub struct WireframePhase {
    pub render_pipeline: wgpu::RenderPipeline,
    pub multisampled_framebuffer: wgpu::TextureView,
}

pub struct WireframePlugin;
impl Plugin for WireframePlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system_to_stage(RendererStage::Init, setup)
            .add_system_to_stage(
                RendererStage::Render,
                render
                    .label(RenderLabel::Wireframe)
                    .after(RenderLabel::Base3d),
            );
    }
}

fn setup(
    mut commands: Commands,
    renderer: Res<WgpuRenderer>,
    mesh_view_layout: Res<MeshViewBindGroupLayout>,
) {
    let shader = renderer
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/wireframe.wgsl"))),
        });

    let pipeline_layout = renderer
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&mesh_view_layout.0],
            push_constant_ranges: &[],
        });

    let multisampled_framebuffer =
        create_multisampled_framebuffer(&renderer.device, &renderer.config, SAMPLE_COUNT as u32);

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
                targets: &[Some(wgpu::ColorTargetState {
                    format: renderer.config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
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

    commands.insert_resource(WireframePhase {
        render_pipeline: pipeline,
        multisampled_framebuffer,
    });
}

fn render(
    phase: Res<WireframePhase>,
    mesh_view_bind_group: Res<MeshViewBindGroup>,
    depth_texture: Res<DepthTexture>,
    mut encoder: ResMut<WgpuEncoder>,
    view: Res<WgpuView>,
    model_query: Query<
        (&Model, &InstanceBuffer, Option<&Instances>),
        (Without<Light>, With<Wireframe>),
    >,
) {
    let encoder = if let Some(encoder) = encoder.0.as_mut() {
        encoder
    } else {
        return;
    };

    let rpass_color_attachment = if SAMPLE_COUNT == 1 {
        wgpu::RenderPassColorAttachment {
            view: &view.0,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            },
        }
    } else {
        wgpu::RenderPassColorAttachment {
            view: &phase.multisampled_framebuffer,
            resolve_target: Some(&view.0),
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
        color_attachments: &[Some(rpass_color_attachment)],
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

    render_pass.set_pipeline(&phase.render_pipeline);

    for (model, instance_buffer, instances) in &model_query {
        render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));
        for mesh in model.meshes.iter() {
            // mesh.vertex_buffer
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(0, &mesh_view_bind_group.0, &[]);
            render_pass.draw_indexed(
                0..mesh.num_elements,
                0,
                0..instances.map(|i| i.0.len() as u32).unwrap_or(1),
            );
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
