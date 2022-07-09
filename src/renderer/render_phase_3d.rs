use bevy::prelude::{Color, Component, QueryState, With, Without, World};
use wgpu::CommandEncoder;

use crate::{
    instances::{InstanceBuffer, Instances},
    light::draw_light_model,
    light::Light,
    mesh::{self},
    model::Model,
    texture::Texture,
    transform::TransformRaw,
};

use super::{
    bind_groups::{
        material::{self, GpuModelMaterials},
        mesh_view::{MeshViewBindGroup, MeshViewBindGroupLayout},
    },
    depth_pass::DepthPass,
    RenderPhase, WgpuRenderer,
};

pub struct DepthTexture(pub Texture);

#[derive(Default)]
pub struct RenderPhase3dDescriptor {
    pub clear_color: Color,
    pub show_depth_buffer: bool,
}

pub struct RenderPhase3d {
    pub opaque_pass: OpaquePass,
}

impl RenderPhase3d {
    pub fn from_world(world: &mut World) -> Self {
        Self {
            opaque_pass: OpaquePass::from_world(world),
        }
    }
}

impl RenderPhase for RenderPhase3d {
    fn update<'w>(&'w mut self, world: &'w mut World) {
        self.opaque_pass.update(world);
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut CommandEncoder) {
        self.opaque_pass.render(world, view, encoder);

        if world
            .resource::<RenderPhase3dDescriptor>()
            .show_depth_buffer
        {
            // TODO the RenderPhase3d should probably own this
            let depth_pass = world.resource::<DepthPass>();
            depth_pass.render(view, encoder);
        }
    }
}

#[derive(Component)]
pub struct Transparent;

#[allow(clippy::type_complexity)]
pub struct OpaquePass {
    pub render_pipeline: wgpu::RenderPipeline,
    pub light_render_pipeline: wgpu::RenderPipeline,
    pub transparent_render_pipeline: wgpu::RenderPipeline,
    pub light_query: QueryState<&'static Model, With<Light>>,
    pub model_query: QueryState<
        (
            &'static Model,
            &'static InstanceBuffer,
            Option<&'static Instances>,
            &'static GpuModelMaterials,
        ),
        (Without<Light>, Without<Transparent>),
    >,
    pub transparent_model_query: QueryState<
        (
            &'static Model,
            &'static InstanceBuffer,
            Option<&'static Instances>,
        ),
        (Without<Light>, With<Transparent>),
    >,
}

impl OpaquePass {
    pub fn from_world(world: &mut World) -> Self {
        let renderer = world.resource::<WgpuRenderer>();
        let mesh_view_layout = world.resource::<MeshViewBindGroupLayout>();

        let render_pipeline_layout =
            renderer
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Render Pipeline Layout"),
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
        );

        Self {
            render_pipeline,
            light_render_pipeline,
            transparent_render_pipeline,
            light_query: world.query_filtered(),
            model_query: world.query_filtered(),
            transparent_model_query: world.query_filtered(),
        }
    }

    pub fn update<'w>(&'w mut self, world: &'w mut World) {
        self.light_query.update_archetypes(world);
        self.model_query.update_archetypes(world);
        self.transparent_model_query.update_archetypes(world);
    }

    fn render(&self, world: &World, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mesh_view_bind_group = world.resource::<MeshViewBindGroup>();
        let depth_texture = world.resource::<DepthTexture>();
        let clear_color = world.resource::<RenderPhase3dDescriptor>().clear_color;

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Opaque Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: clear_color.r() as f64,
                        g: clear_color.g() as f64,
                        b: clear_color.b() as f64,
                        a: clear_color.a() as f64,
                    }),
                    store: true,
                },
            }],
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
        render_pass.set_pipeline(&self.render_pipeline);
        for (model, instance_buffer, instances, gpu_materials) in
            self.model_query.iter_manual(world)
        {
            // The draw function also uses the instance buffer under the hood it simply is of size 1
            render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));
            let transparent = false;
            if let Some(instances) = instances {
                model.draw_instanced(
                    &mut render_pass,
                    0..instances.0.len() as u32,
                    gpu_materials,
                    &mesh_view_bind_group.0,
                    transparent,
                );
            } else {
                model.draw(
                    &mut render_pass,
                    gpu_materials,
                    &mesh_view_bind_group.0,
                    transparent,
                );
            }
        }

        // TODO I need a better way to identify transparent meshes in a model
        render_pass.set_pipeline(&self.transparent_render_pipeline);
        for (model, instance_buffer, instances, gpu_materials) in
            self.model_query.iter_manual(world)
        {
            // The draw function also uses the instance buffer under the hood it simply is of size 1
            render_pass.set_vertex_buffer(1, instance_buffer.0.slice(..));
            let transparent = true;
            if let Some(instances) = instances {
                model.draw_instanced(
                    &mut render_pass,
                    0..instances.0.len() as u32,
                    gpu_materials,
                    &mesh_view_bind_group.0,
                    transparent,
                );
            } else {
                model.draw(
                    &mut render_pass,
                    gpu_materials,
                    &mesh_view_bind_group.0,
                    transparent,
                );
            }
        }

        render_pass.set_pipeline(&self.light_render_pipeline);
        for light_model in self.light_query.iter_manual(world) {
            draw_light_model(&mut render_pass, light_model, &mesh_view_bind_group.0);
        }
    }
}
