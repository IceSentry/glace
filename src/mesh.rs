use bevy::math::{Vec2, Vec3};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub tangent: Vec3,
    pub bitangent: Vec3,
}

impl Vertex {
    pub fn new(position: Vec3, normal: Vec3, uv: Vec2) -> Self {
        Self {
            position,
            normal,
            uv,
            tangent: Vec3::ZERO,
            bitangent: Vec3::ZERO,
        }
    }

    pub fn from_arrays(position: [f32; 3], normal: [f32; 3], uv: [f32; 2]) -> Self {
        Self {
            position: Vec3::from(position),
            normal: Vec3::from(normal),
            uv: Vec2::from(uv),
            tangent: Vec3::ZERO,
            bitangent: Vec3::ZERO,
        }
    }

    pub fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTESS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2,
            3 => Float32x3,
            4 => Float32x3
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTESS,
        }
    }
}

// TODO use Map for attributes
#[derive(Debug)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Option<Vec<u32>>,
    pub material_id: Option<usize>,
}

impl Mesh {
    pub fn compute_normals(&mut self) {
        fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
            let (a, b, c) = (Vec3::from(a), Vec3::from(b), Vec3::from(c));
            (b - a).cross(c - a).normalize().into()
        }

        if let Some(indices) = self.indices.as_ref() {
            for v in self.vertices.iter_mut() {
                v.normal = Vec3::ZERO;
            }

            for i in indices.chunks_exact(3) {
                if let [i1, i2, i3] = i {
                    let v_a = self.vertices[*i1 as usize];
                    let v_b = self.vertices[*i2 as usize];
                    let v_c = self.vertices[*i3 as usize];

                    let edge_ab = v_b.position - v_a.position;
                    let edge_ac = v_c.position - v_a.position;

                    let normal = edge_ab.cross(edge_ac);

                    self.vertices[*i1 as usize].normal += normal;
                    self.vertices[*i2 as usize].normal += normal;
                    self.vertices[*i3 as usize].normal += normal;
                }
            }

            for v in self.vertices.iter_mut() {
                v.normal = v.normal.normalize();
            }
        } else {
            let mut normals = vec![];
            for v in self.vertices.chunks_exact_mut(3) {
                if let [v1, v2, v3] = v {
                    let normal = face_normal(
                        v1.position.to_array(),
                        v2.position.to_array(),
                        v3.position.to_array(),
                    );
                    normals.push(normal);
                }
            }
        }
    }

    pub fn compute_tangents(&mut self) {
        if let Some(indices) = self.indices.as_ref() {
            let mut triangles_included = (0..self.vertices.len()).collect::<Vec<_>>();
            for c in indices.chunks(3) {
                let v0 = self.vertices[c[0] as usize];
                let v1 = self.vertices[c[1] as usize];
                let v2 = self.vertices[c[2] as usize];

                let pos0 = v0.position;
                let pos1 = v1.position;
                let pos2 = v2.position;

                let uv0 = v0.uv;
                let uv1 = v1.uv;
                let uv2 = v2.uv;

                // Calculate the edges of the triangle
                let delta_pos1 = pos1 - pos0;
                let delta_pos2 = pos2 - pos0;

                // This will give us a direction to calculate the
                // tangent and bitangent
                let delta_uv1 = uv1 - uv0;
                let delta_uv2 = uv2 - uv0;

                // Solving the following system of equations will
                // give us the tangent and bitangent.
                //     delta_pos1 = delta_uv1.x * T + delta_u.y * B
                //     delta_pos2 = delta_uv2.x * T + delta_uv2.y * B
                // Luckily, the place I found this equation provided
                // the solution!
                let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
                // We flip the bitangent to enable right-handed normal
                // maps with wgpu texture coordinate system
                let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * -r;

                // We'll use the same tangent/bitangent for each vertex in the triangle
                self.vertices[c[0] as usize].tangent += tangent;
                self.vertices[c[1] as usize].tangent += tangent;
                self.vertices[c[2] as usize].tangent += tangent;

                self.vertices[c[0] as usize].bitangent += bitangent;
                self.vertices[c[1] as usize].bitangent += bitangent;
                self.vertices[c[2] as usize].bitangent += bitangent;

                // Used to average the tangents/bitangents
                triangles_included[c[0] as usize] += 1;
                triangles_included[c[1] as usize] += 1;
                triangles_included[c[2] as usize] += 1;
            }

            // Average the tangents/bitangents
            for (i, n) in triangles_included.into_iter().enumerate() {
                let denom = 1.0 / n as f32;
                let v = &mut self.vertices[i];
                v.tangent = (v.tangent * denom).normalize();
                v.bitangent = (v.bitangent * denom).normalize();
            }
        } else {
            todo!("tangents only computed for indexed meshes");
        }
    }
}
