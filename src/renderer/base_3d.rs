use bevy::ecs::prelude::*;

use super::{
    bind_groups::material::{self, GpuModelMaterials},
    DepthTexture, GlaceClearColor, Msaa, WgpuEncoder, WgpuRenderer, WgpuView,
};

use crate::renderer::bind_groups::mesh_view::{MeshViewBindGroup, MeshViewBindGroupLayout};
use crate::{
    instances::{InstanceBuffer, Instances},
    light::{draw_light_model, Light},
    mesh,
    model::Model,
    texture::Texture,
    transform::TransformRaw,
};

#[derive(Component)]
pub struct Transparent;

#[derive(Resource)]
pub struct Base3dPass {
    render_pipeline: wgpu::RenderPipeline,
    light_render_pipeline: wgpu::RenderPipeline,
    transparent_render_pipeline: wgpu::RenderPipeline,
}

impl Base3dPass {
    fn new(
        renderer: &WgpuRenderer,
        mesh_view_layout: &MeshViewBindGroupLayout,
        sample_count: u32,
    ) -> Self {
        let render_pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("base_3d Pipeline Layout"),
                    bind_group_layouts: &[
                        &mesh_view_layout.0,
                        &material::bind_group_layout(&renderer.device),
                    ],
                    push_constant_ranges: &[],
                });

        // TODO have a better way to attach draw commands to a pipeline
        let render_pipeline = renderer.create_render_pipeline(
            "Opaque Render Pipeline",
            include_str!("shaders/shader.wgsl"),
            &render_pipeline_layout,
            &[mesh::Vertex::layout(), TransformRaw::layout()],
            Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            wgpu::BlendState::REPLACE,
            sample_count,
        );

        let transparent_render_pipeline = renderer.create_render_pipeline(
            "Transparent Render Pipeline",
            include_str!("shaders/shader.wgsl"),
            &render_pipeline_layout,
            &[mesh::Vertex::layout(), TransformRaw::layout()],
            Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            wgpu::BlendState::ALPHA_BLENDING,
            sample_count,
        );

        let light_render_pipeline = renderer.create_render_pipeline(
            "Light Render Pipeline",
            include_str!("shaders/light.wgsl"),
            &renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Light Pipeline Layout"),
                    bind_group_layouts: &[&mesh_view_layout.0],
                    push_constant_ranges: &[],
                }),
            &[mesh::Vertex::layout()],
            Some(wgpu::DepthStencilState {
                format: Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            wgpu::BlendState::REPLACE,
            sample_count,
        );

        Self {
            render_pipeline,
            light_render_pipeline,
            transparent_render_pipeline,
        }
    }
}

pub fn setup(
    mut commands: Commands,
    renderer: Res<WgpuRenderer>,
    mesh_view_layout: Res<MeshViewBindGroupLayout>,
    msaa: Res<Msaa>,
) {
    commands.insert_resource(Base3dPass::new(&renderer, &mesh_view_layout, msaa.samples));
}

pub fn update_render_pass(
    mut render_pass: ResMut<Base3dPass>,
    msaa: Res<Msaa>,
    mesh_view_layout: Res<MeshViewBindGroupLayout>,
    renderer: Res<WgpuRenderer>,
) {
    if msaa.is_changed() {
        log::info!("updating base_3d render pass");
        *render_pass = Base3dPass::new(&renderer, &mesh_view_layout, msaa.samples);
    }
}

pub fn render(
    mesh_view_bind_group: Res<MeshViewBindGroup>,
    depth_texture: Res<DepthTexture>,
    mut encoder: ResMut<WgpuEncoder>,
    view: Res<WgpuView>,
    pass: Res<Base3dPass>,
    light_query: Query<&Model, With<Light>>,
    model_query: Query<
        (
            &Model,
            &InstanceBuffer,
            Option<&Instances>,
            &GpuModelMaterials,
        ),
        (Without<Light>, Without<Transparent>),
    >,
    clear_color: Res<GlaceClearColor>,
) {
    let encoder = if let Some(encoder) = encoder.0.as_mut() {
        encoder
    } else {
        return;
    };

    // log::info!("render base");

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Base 3d Render Pass"),
        color_attachments: &[Some(view.get_color_attachment(wgpu::Operations {
            load: wgpu::LoadOp::Clear(clear_color.0.into()),
            store: true,
        }))],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth_texture.0.view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: true,
            }),
            stencil_ops: None,
        }),
    });

    // TODO figure out how to sort models
    render_pass.set_pipeline(&pass.render_pipeline);
    for (model, instance_buffer, instances, gpu_materials) in &model_query {
        // The draw function also uses the instance buffer under the hood it simply is of size 1
        render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));
        model.draw_instanced(
            &mut render_pass,
            0..instances.map(|i| i.0.len() as u32).unwrap_or(1),
            gpu_materials,
            &mesh_view_bind_group.0,
            false,
        );
    }

    // TODO I need a better way to identify transparent meshes in a model
    render_pass.set_pipeline(&pass.transparent_render_pipeline);
    for (model, instance_buffer, instances, gpu_materials) in &model_query {
        // The draw function also uses the instance buffer under the hood it simply is of size 1
        render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));
        model.draw_instanced(
            &mut render_pass,
            0..instances.map(|i| i.0.len() as u32).unwrap_or(1),
            gpu_materials,
            &mesh_view_bind_group.0,
            true,
        );
    }

    render_pass.set_pipeline(&pass.light_render_pipeline);
    for light_model in &light_query {
        draw_light_model(&mut render_pass, light_model, &mesh_view_bind_group.0);
    }
}
