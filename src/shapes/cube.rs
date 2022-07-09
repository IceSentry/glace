use crate::{
    mesh::{Mesh, Vertex},
    model::ModelMesh,
};

#[derive(Debug, Copy, Clone)]
pub struct Cube {
    pub min_x: f32,
    pub max_x: f32,

    pub min_y: f32,
    pub max_y: f32,

    pub min_z: f32,
    pub max_z: f32,
}

impl Cube {
    pub fn new(x_length: f32, y_length: f32, z_length: f32) -> Cube {
        Cube {
            max_x: x_length / 2.0,
            min_x: -x_length / 2.0,
            max_y: y_length / 2.0,
            min_y: -y_length / 2.0,
            max_z: z_length / 2.0,
            min_z: -z_length / 2.0,
        }
    }

    pub fn mesh(&self, device: &wgpu::Device) -> ModelMesh {
        #[rustfmt::skip]
        let vertices = vec![
            // Top
            ([self.min_x, self.min_y, self.max_z], [0., 0., 1.0], [0., 0.]),
            ([self.max_x, self.min_y, self.max_z], [0., 0., 1.0], [1.0, 0.]),
            ([self.max_x, self.max_y, self.max_z], [0., 0., 1.0], [1.0, 1.0]),
            ([self.min_x, self.max_y, self.max_z], [0., 0., 1.0], [0., 1.0]),
            // Bottom
            ([self.min_x, self.max_y, self.min_z], [0., 0., -1.0], [1.0, 0.]),
            ([self.max_x, self.max_y, self.min_z], [0., 0., -1.0], [0., 0.]),
            ([self.max_x, self.min_y, self.min_z], [0., 0., -1.0], [0., 1.0]),
            ([self.min_x, self.min_y, self.min_z], [0., 0., -1.0], [1.0, 1.0]),
            // Right
            ([self.max_x, self.min_y, self.min_z], [1.0, 0., 0.], [0., 0.]),
            ([self.max_x, self.max_y, self.min_z], [1.0, 0., 0.], [1.0, 0.]),
            ([self.max_x, self.max_y, self.max_z], [1.0, 0., 0.], [1.0, 1.0]),
            ([self.max_x, self.min_y, self.max_z], [1.0, 0., 0.], [0., 1.0]),
            // Left
            ([self.min_x, self.min_y, self.max_z], [-1.0, 0., 0.], [1.0, 0.]),
            ([self.min_x, self.max_y, self.max_z], [-1.0, 0., 0.], [0., 0.]),
            ([self.min_x, self.max_y, self.min_z], [-1.0, 0., 0.], [0., 1.0]),
            ([self.min_x, self.min_y, self.min_z], [-1.0, 0., 0.], [1.0, 1.0]),
            // Front
            ([self.max_x, self.max_y, self.min_z], [0., 1.0, 0.], [1.0, 0.]),
            ([self.min_x, self.max_y, self.min_z], [0., 1.0, 0.], [0., 0.]),
            ([self.min_x, self.max_y, self.max_z], [0., 1.0, 0.], [0., 1.0]),
            ([self.max_x, self.max_y, self.max_z], [0., 1.0, 0.], [1.0, 1.0]),
            // Back
            ([self.max_x, self.min_y, self.max_z], [0., -1.0, 0.], [0., 0.]),
            ([self.min_x, self.min_y, self.max_z], [0., -1.0, 0.], [1.0, 0.]),
            ([self.min_x, self.min_y, self.min_z], [0., -1.0, 0.], [1.0, 1.0]),
            ([self.max_x, self.min_y, self.min_z], [0., -1.0, 0.], [0., 1.0]),
        ].iter()
        .map(|(position, normal, uv)| Vertex::from_arrays(*position, *normal, *uv))
        .collect::<Vec<_>>();

        let indices: Vec<u32> = vec![
            0, 1, 2, 2, 3, 0, // top
            4, 5, 6, 6, 7, 4, // bottom
            8, 9, 10, 10, 11, 8, // right
            12, 13, 14, 14, 15, 12, // left
            16, 17, 18, 18, 19, 16, // front
            20, 21, 22, 22, 23, 20, // back
        ];

        ModelMesh::from_mesh(
            "cube",
            device,
            &Mesh {
                vertices,
                indices: Some(indices),
                material_id: None,
            },
        )
    }
}
