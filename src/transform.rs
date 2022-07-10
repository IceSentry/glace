use bevy::{
    math::{Mat3, Mat4, Quat, Vec3},
    prelude::Component,
};

#[derive(Component)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            rotation: Quat::default(),
            translation: Vec3::ZERO,
            scale: Vec3::ONE,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TransformRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
    inverse_transpose_model: [[f32; 4]; 4],
}

impl Transform {
    pub fn to_raw(&self) -> TransformRaw {
        let model =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
        TransformRaw {
            model: model.to_cols_array_2d(),
            normal: Mat3::from_quat(self.rotation).to_cols_array_2d(),
            inverse_transpose_model: model.inverse().transpose().to_cols_array_2d(),
        }
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
