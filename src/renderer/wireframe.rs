use std::borrow::Cow;

use bevy::{app::prelude::*, ecs::prelude::*, utils::prelude::*};

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
    DepthTexture, Msaa, WgpuEncoder, WgpuRenderer, WgpuView,
};

#[derive(Component)]
pub struct Wireframe;

#[derive(Resource)]
pub struct WireframePhase {
    pub render_pipeline: wgpu::RenderPipeline,
}

pub struct WireframePlugin;
impl Plugin for WireframePlugin {
    fn build(&self, _app: &mut App) {
        // app.add_startup_system_to_stage(RendererStage::Init, setup)
        //     .add_system_to_stage(
        //         RendererStage::Render,
        //         render
        //             .label(RenderLabel::Wireframe)
        //             .after(RenderLabel::Base3d),
        //     );
    }
}

fn _setup(
    mut commands: Commands,
    renderer: Res<WgpuRenderer>,
    mesh_view_layout: Res<MeshViewBindGroupLayout>,
    msaa: Res<Msaa>,
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    slope_scale: -1.0,
                    ..default()
                },
            }),
            multisample: wgpu::MultisampleState {
                count: msaa.samples,
                ..default()
            },
            multiview: None,
        });

    commands.insert_resource(WireframePhase {
        render_pipeline: pipeline,
    });
}

fn _render(
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

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(view.get_color_attachment(wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: true,
        }))],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth_texture.0.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            }),
            stencil_ops: None,
        }),
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
