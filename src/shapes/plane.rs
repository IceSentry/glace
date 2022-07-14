use crate::{
    mesh::{Mesh, Vertex},
    model::ModelMesh,
};

#[derive(Debug, Copy, Clone)]
pub struct Plane {
    pub resolution: usize,
    pub size: f32,
}

impl Default for Plane {
    fn default() -> Self {
        Plane {
            resolution: 10,
            size: 1.0,
        }
    }
}

impl Plane {
    #[allow(unused)]
    pub fn mesh(&self, device: &wgpu::Device) -> ModelMesh {
        let mut vertices = Vec::with_capacity((self.resolution + 1) * (self.resolution + 1));
        let resolution_modifier = self.size / self.resolution as f32;
        for y in 0..=self.resolution {
            for x in 0..=self.resolution {
                vertices.push((
                    [
                        x as f32 * resolution_modifier,
                        0.0,
                        y as f32 * resolution_modifier,
                    ],
                    [0.0, 1.0, 0.0],
                    [
                        x as f32 / self.resolution as f32,
                        y as f32 / self.resolution as f32,
                    ],
                ));
            }
        }
        let mut indices = vec![0; self.resolution * self.resolution * 6];
        let mut triangle_index = 0;
        let mut vertex_index = 0;
        for _y in 0..self.resolution {
            for _x in 0..self.resolution {
                indices[triangle_index] = vertex_index;
                indices[triangle_index + 1] = vertex_index + self.resolution as u32 + 1;
                indices[triangle_index + 2] = vertex_index + 1;

                indices[triangle_index + 3] = vertex_index + 1;
                indices[triangle_index + 4] = vertex_index + self.resolution as u32 + 1;
                indices[triangle_index + 5] = vertex_index + self.resolution as u32 + 2;

                vertex_index += 1;
                triangle_index += 6;
            }
            vertex_index += 1;
        }

        let vertices: Vec<_> = vertices
            .iter()
            .map(|(position, normal, uv)| Vertex::from_arrays(*position, *normal, *uv))
            .collect();

        let mut mesh = Mesh {
            vertices,
            indices: Some(indices),
            material_id: None,
        };
        mesh.compute_tangents();

        ModelMesh::from_mesh("plane", device, &mesh)
    }
}
