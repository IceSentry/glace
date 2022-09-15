use bevy::{ecs::prelude::*, math::prelude::*, render::color::Color};
use std::ops::Range;

use crate::model::{Model, ModelMesh};

#[derive(Component)]
pub struct Light {
    pub position: Vec3,
    pub color: Color,
}

#[allow(unused)]
fn draw_light_mesh<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    mesh: &'a ModelMesh,
    mesh_view_bind_group: &'a wgpu::BindGroup,
) {
    draw_light_mesh_instanced(render_pass, mesh, 0..1, mesh_view_bind_group);
}

fn draw_light_mesh_instanced<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    mesh: &'a ModelMesh,
    instances: Range<u32>,
    mesh_view_bind_group: &'a wgpu::BindGroup,
) {
    render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
    render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    render_pass.set_bind_group(0, mesh_view_bind_group, &[]);
    render_pass.draw_indexed(0..mesh.num_elements, 0, instances);
}

pub fn draw_light_model<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    model: &'a Model,
    mesh_view_bind_group: &'a wgpu::BindGroup,
) {
    draw_light_model_instanced(render_pass, model, 0..1, mesh_view_bind_group);
}

fn draw_light_model_instanced<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    model: &'a Model,
    instances: Range<u32>,
    mesh_view_bind_group: &'a wgpu::BindGroup,
) {
    for mesh in &model.meshes {
        draw_light_mesh_instanced(render_pass, mesh, instances.clone(), mesh_view_bind_group);
    }
}
