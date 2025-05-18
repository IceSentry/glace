use bevy::{
    log::info,
    math::{Vec3, Vec4},
};

use crate::Vertex;

pub struct GeoSurface {
    pub start_index: u32,
    pub count: u32,
}

pub struct MeshAsset {
    pub name: Option<String>,
    pub surfaces: Vec<GeoSurface>,
    pub indices: Vec<u32>,
    pub vertices: Vec<Vertex>,
}

pub fn load_gltf(path: &str, override_colors: bool) -> Vec<MeshAsset> {
    info!("Loading gltf: {path:?}");
    let (gltf, buffers, _) = gltf::import(path).expect("Failed to import gltf");
    let mut indices: Vec<u32> = vec![];
    let mut vertices: Vec<Vertex> = vec![];
    let mut meshes = vec![];
    for mesh in gltf.meshes() {
        indices.clear();
        vertices.clear();

        let mut surfaces = vec![];
        for p in mesh.primitives() {
            let new_surface = GeoSurface {
                start_index: indices.len() as u32,
                count: p.indices().expect("Mesh should have indices").count() as u32,
            };
            surfaces.push(new_surface);

            let reader = p.reader(|buffer| Some(&buffers[buffer.index()]));
            let initial_vtx = vertices.len();

            // load indices
            {
                let indices_reader = reader.read_indices().expect("Mesh should have indices");
                let indices_count = p.indices().expect("Mesh should have indices").count();
                indices.resize(indices.len() + indices_count, 0);
                for idx in indices_reader.into_u32() {
                    indices.push(idx + initial_vtx as u32);
                }
            }

            // load vertex positions
            {
                let positions_reader = reader.read_positions().expect("Mesh should have positions");
                let pos_count = positions_reader.clone().count();
                vertices.resize(vertices.len() + pos_count, Vertex::default());
                for (index, v) in positions_reader.enumerate() {
                    vertices[initial_vtx + index] = Vertex {
                        position: Vec3::from_array(v),
                        uv_x: 0.0,
                        normal: Vec3::new(1.0, 0.0, 0.0),
                        uv_y: 0.0,
                        color: Vec4::splat(1.0),
                    };
                }
            }

            // load normals
            if let Some(normals_reader) = reader.read_normals() {
                for (index, n) in normals_reader.enumerate() {
                    vertices[initial_vtx + index].normal = Vec3::from_array(n);
                }
            }

            // load uvs
            if let Some(tex_coord_0_reader) = reader.read_tex_coords(0) {
                for (index, uv) in tex_coord_0_reader.into_f32().enumerate() {
                    vertices[initial_vtx + index].uv_x = uv[0];
                    vertices[initial_vtx + index].uv_y = uv[1];
                }
            }

            // load vertex colors
            if let Some(colors_reader) = reader.read_colors(0) {
                for (index, color) in colors_reader.into_rgba_f32().enumerate() {
                    vertices[initial_vtx + index].color = Vec4::from_array(color);
                }
            }
        }
        if override_colors {
            for vtx in &mut vertices {
                vtx.color = vtx.normal.extend(1.0);
            }
        }
        meshes.push(MeshAsset {
            name: mesh.name().map(|s| s.to_owned()),
            surfaces,
            indices: indices.clone(),
            vertices: vertices.clone(),
        });
    }
    meshes
}
