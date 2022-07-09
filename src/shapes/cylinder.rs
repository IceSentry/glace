use bevy::math::Vec3;

use crate::{
    mesh::{Mesh, Vertex},
    model::ModelMesh,
};

/// A cylinder which stands on the XZ plane
pub struct Cylinder {
    /// Radius of the cylinder (X&Z axis)
    pub radius: f32,
    /// Height of the cylinder (Y axis)
    pub height: f32,
    /// Number of vertices around each horizontal slice of the cylinder
    pub resolution: u32,
    /// Number of vertical subdivisionss
    pub subdivisions: u32,
}

impl Default for Cylinder {
    fn default() -> Self {
        Self {
            radius: 0.5,
            height: 1.0,
            resolution: 20,
            subdivisions: 4,
        }
    }
}

impl Cylinder {
    #[allow(unused)]
    pub fn mesh(&self, device: &wgpu::Device) -> ModelMesh {
        assert!(
            self.radius > 0.0 && self.height > 0.0 && self.resolution > 0 && self.subdivisions > 0
        );

        let count = (self.resolution * (self.subdivisions + 3) + 2) as usize;
        let mut positions = Vec::with_capacity(count);
        let step = std::f32::consts::PI * 2.0 / self.resolution as f32;
        let mut add_ring = |height, with_center| {
            if with_center {
                positions.push([0.0, height, 0.0]);
            }
            for j in 0..self.resolution {
                let theta = step * j as f32;
                positions.push([theta.cos() * self.radius, height, theta.sin() * self.radius]);
            }
        };

        // Shaft vertices
        let h_step = self.height / self.subdivisions as f32;
        for i in 0..=self.subdivisions {
            add_ring(self.height * 0.5 - h_step * i as f32, false);
        }

        // Top vertices
        let top_offset = self.resolution * (self.subdivisions + 1);
        add_ring(self.height * 0.5, true);

        // Bottom vertices
        let bottom_offset = top_offset + self.resolution + 1;
        add_ring(-self.height * 0.5, true);
        assert_eq!(positions.len(), count);

        let index_count =
            ((6 * self.subdivisions * self.resolution) + 6 * self.resolution) as usize;
        let mut indices = Vec::with_capacity(index_count);

        // Shaft quads
        for i in 0..self.subdivisions {
            let base1 = self.resolution * i;
            let base2 = base1 + self.resolution;
            for j in 0..self.resolution {
                let j1 = (j + 1) % self.resolution;
                indices.extend([base1 + j, base1 + j1, base2 + j].iter().copied());
                indices.extend([base1 + j1, base2 + j1, base2 + j].iter().copied());
            }
        }

        // Top circle
        for j in 0..self.resolution {
            let j1 = (j + 1) % self.resolution;
            let base = top_offset + 1;
            indices.extend([base + j1, base + j, top_offset].iter().copied());
        }
        // Bottom circle
        for j in 0..self.resolution {
            let j1 = (j + 1) % self.resolution;
            let base = bottom_offset + 1;
            indices.extend([base + j, base + j1, bottom_offset].iter().copied());
        }
        assert_eq!(indices.len(), index_count);

        let mut normals: Vec<[f32; 3]> = positions
            .iter()
            .map(|&p| {
                (Vec3::from(p) * Vec3::new(1.0, 0.0, 1.0))
                    .normalize()
                    .into()
            })
            .collect();

        for i in top_offset..bottom_offset {
            normals[i as usize] = [0.0, 1.0, 0.0];
        }
        for i in bottom_offset..count as u32 {
            normals[i as usize] = [0.0, -1.0, 0.0];
        }

        let uvs: Vec<[f32; 2]> = positions
            .iter()
            .map(|&p| {
                [
                    p[0] / self.radius,
                    (p[1] + self.height) / (self.height * 2.0),
                ]
            })
            .collect();

        let mut vertices = Vec::new();
        for (i, position) in positions.iter().enumerate() {
            vertices.push(Vertex::from_arrays(*position, normals[i], uvs[i]));
        }

        ModelMesh::from_mesh(
            "cylinder",
            device,
            &Mesh {
                vertices,
                indices: Some(indices),
                material_id: None,
            },
        )
    }
}
