use bevy::{prelude::*, render::render_resource::ShaderType};
use wgpu::BufferUsages;

use crate::buffer_vec::BufferVec;

pub struct MeshPlugin;
impl Plugin for MeshPlugin {
    fn build(&self, app: &mut App) {
        // app.add_systems(PostUpdate, convert_mesh);
    }
}

#[derive(Default, Clone, Copy, ShaderType)]
pub struct Vertex {
    pub position: Vec3,
    pub uv_x: f32,
    pub normal: Vec3,
    pub uv_y: f32,
    pub color: Vec4,
}

impl Vertex {
    pub fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTESS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32,
            2 => Float32x3,
            3 => Float32,
            4 => Float32x4,
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTESS,
        }
    }
}
#[derive(Component)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

fn bevy_mesh_to_glace_mesh(bevy_mesh: &bevy::render::mesh::Mesh) -> Mesh {
    let mut indices = vec![];
    let mut vertices = vec![];
    if let Some(mesh_indices) = bevy_mesh.indices() {
        for index in mesh_indices.iter() {
            indices.push(index as u32);
        }
    }
    if let Some(positions) = bevy_mesh.attribute(bevy::render::mesh::Mesh::ATTRIBUTE_POSITION) {
        for pos in positions.as_float3().unwrap() {
            vertices.push(Vertex {
                position: Vec3::new(pos[0], pos[1], pos[2]),
                uv_x: 0.0,
                normal: Vec3::ZERO,
                uv_y: 0.0,
                color: Vec4::ZERO,
            });
        }
    }
    Mesh { vertices, indices }
}

fn convert_mesh(
    mut commands: Commands,
    meshes: Res<Assets<bevy::render::mesh::Mesh>>,
    added_meshes: Query<(Entity, &Mesh3d), Added<Mesh3d>>,
) {
    for (entity, mesh) in &added_meshes {
        let Some(mesh) = meshes.get(mesh) else {
            continue;
        };
        let mesh = bevy_mesh_to_glace_mesh(mesh);
        // TODO consider uploading meshes here
        commands.entity(entity).insert(mesh);
    }
}

pub struct GpuMeshBuffers {
    pub index_buffer: BufferVec<u32>,
    pub vertex_buffer: BufferVec<Vertex>,
}

pub fn upload_mesh(device: &wgpu::Device, queue: &wgpu::Queue, mesh: &Mesh) -> GpuMeshBuffers {
    let mut vertex_buffer = BufferVec::new(BufferUsages::STORAGE | BufferUsages::VERTEX);
    vertex_buffer.reserve(mesh.vertices.len(), device);
    for vertex in mesh.vertices.iter().copied() {
        vertex_buffer.push(vertex);
    }
    vertex_buffer.write_buffer(device, queue);

    let mut index_buffer = BufferVec::new(BufferUsages::INDEX);
    index_buffer.reserve(mesh.indices.len(), device);
    for index in mesh.indices.iter().copied() {
        index_buffer.push(index);
    }
    index_buffer.write_buffer(device, queue);

    GpuMeshBuffers {
        vertex_buffer,
        index_buffer,
    }
}
