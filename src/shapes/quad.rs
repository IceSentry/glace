use crate::{
    mesh::{Mesh, Vertex},
    model::ModelMesh,
};

#[derive(Debug, Copy, Clone)]
pub struct Quad;

impl Quad {
    #[allow(unused)]
    pub fn mesh(&self, device: &wgpu::Device) -> ModelMesh {
        let mut vertices = vec![
            ([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]), // 0
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]), // 1
            ([1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0]), // 2
            ([0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0]), // 3
        ];

        let indices = vec![
            0, 1, 3, // 1.
            2, 3, 1, // 2.
        ];

        let vertices: Vec<_> = vertices
            .iter()
            .map(|(position, normal, uv)| Vertex::from_arrays(*position, *normal, *uv))
            .collect();

        let mut mesh = Mesh {
            vertices,
            indices: Some(indices),
            material_id: None,
        };

        ModelMesh::from_mesh("quad", device, &mesh)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct FullscreenQuad;

impl FullscreenQuad {
    #[allow(unused)]
    pub fn mesh(&self, device: &wgpu::Device) -> ModelMesh {
        let mut vertices = vec![
            ([-1.0, -1.0, 0.0], [0.0, 0.0, 0.0], [0.0, 1.0]), // 0
            ([1.0, -1.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0]),  // 1
            ([1.0, 1.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0]),   // 2
            ([-1.0, 1.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0]),  // 3
        ];

        let indices = vec![
            0, 1, 2, // 1.
            0, 2, 3, // 2.
        ];

        let vertices: Vec<_> = vertices
            .iter()
            .map(|(position, normal, uv)| Vertex::from_arrays(*position, *normal, *uv))
            .collect();

        let mut mesh = Mesh {
            vertices,
            indices: Some(indices),
            material_id: None,
        };

        ModelMesh::from_mesh("quad", device, &mesh)
    }
}
