use crate::{
    image_utils::image_from_color, mesh::Mesh, renderer::bind_groups::material::GpuModelMaterials,
};
use bevy::{
    math::{Vec3, Vec4},
    prelude::{Color, Component},
};
use image::RgbaImage;
use std::ops::Range;
use wgpu::util::DeviceExt;

#[derive(Component)]
pub struct Model {
    pub meshes: Vec<ModelMesh>,
    pub materials: Vec<Material>,
}

impl Model {
    #[allow(unused)]
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        gpu_materials: &'a GpuModelMaterials,
        mesh_view_bind_group: &'a wgpu::BindGroup,
        transparent: bool,
    ) {
        self.draw_instanced(
            render_pass,
            0..1,
            gpu_materials,
            mesh_view_bind_group,
            transparent,
        );
    }

    pub fn draw_instanced<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        instances: Range<u32>,
        gpu_materials: &'a GpuModelMaterials,
        mesh_view_bind_group: &'a wgpu::BindGroup,
        transparent: bool,
    ) {
        for mesh in &self.meshes {
            // TODO get data from Handle
            // TODO handle material_id == None
            let material = &gpu_materials.data[mesh.material_id.unwrap_or(0)];

            if transparent && material.0.alpha < 1.0 {
                mesh.draw_instanced(
                    render_pass,
                    instances.clone(),
                    &material.2,
                    mesh_view_bind_group,
                );
            }

            if !transparent && material.0.alpha == 1.0 {
                mesh.draw_instanced(
                    render_pass,
                    instances.clone(),
                    &material.2,
                    mesh_view_bind_group,
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub base_color: Vec4,
    pub alpha: f32,
    pub gloss: f32,
    pub specular: Vec3,
    pub diffuse_texture: RgbaImage,
    pub normal_texture: Option<RgbaImage>,
    pub specular_texture: Option<RgbaImage>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name: "Default Material".to_string(),
            base_color: Color::WHITE.as_rgba_f32().into(),
            alpha: 1.0,
            gloss: 1.0,
            specular: Vec3::ONE,
            diffuse_texture: image_from_color(Color::WHITE),
            normal_texture: None,
            specular_texture: None,
        }
    }
}

impl Material {
    #[allow(unused)]
    pub fn from_color(color: Color) -> Self {
        Self {
            name: "Color Material".to_string(),
            base_color: color.as_rgba_f32().into(),
            diffuse_texture: image_from_color(color),
            alpha: color.a(),
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub struct ModelMesh {
    pub name: String,
    // TODO don't store buffer on mesh
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material_id: Option<usize>,
}

impl ModelMesh {
    pub fn from_mesh(label: &str, device: &wgpu::Device, mesh: &Mesh) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} vertex buffer")),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} index buffer")),
            contents: bytemuck::cast_slice(
                mesh.indices
                    .as_ref()
                    .expect("tried to get index buffer without indices"),
            ),
            usage: wgpu::BufferUsages::INDEX,
        });

        ModelMesh {
            name: label.to_string(),
            vertex_buffer,
            index_buffer,
            num_elements: mesh.indices.clone().map(|i| i.len() as u32).unwrap_or(1),
            material_id: mesh.material_id,
        }
    }

    #[allow(unused)]
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        material_bind_group: &'a wgpu::BindGroup,
        mesh_view_bind_group: &'a wgpu::BindGroup,
    ) {
        self.draw_instanced(render_pass, 0..1, material_bind_group, mesh_view_bind_group);
    }

    pub fn draw_instanced<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        instances: Range<u32>,
        material_bind_group: &'a wgpu::BindGroup,
        mesh_view_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(0, mesh_view_bind_group, &[]);
        render_pass.set_bind_group(1, material_bind_group, &[]);
        render_pass.draw_indexed(0..self.num_elements, 0, instances);
    }
}
