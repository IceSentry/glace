use bevy::{math::Mat3, prelude::Transform};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TransformRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
    inverse_transpose_model: [[f32; 4]; 4],
}

pub fn to_raw(transform: &Transform) -> TransformRaw {
    let model = transform.compute_matrix();
    TransformRaw {
        model: model.to_cols_array_2d(),
        normal: Mat3::from_quat(transform.rotation).to_cols_array_2d(),
        inverse_transpose_model: model.inverse().transpose().to_cols_array_2d(),
    }
}

impl TransformRaw {
    pub fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTESS: [wgpu::VertexAttribute; 11] = wgpu::vertex_attr_array![
            // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
            // for each vec4. We'll have to reassemble the mat4 in
            // the shader.

            // model
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4,
            // normal
            9  => Float32x3,
            10 => Float32x3,
            11 => Float32x3,
            // inverse_transpose_model
            12 => Float32x4,
            13 => Float32x4,
            14 => Float32x4,
            15 => Float32x4,
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TransformRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTESS,
        }
    }
}
