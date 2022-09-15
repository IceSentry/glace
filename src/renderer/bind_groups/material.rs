use bevy::{
    ecs::prelude::*,
    math::prelude::*,
    render::color::Color,
    render::render_resource::{encase::UniformBuffer, ShaderType},
};
use wgpu::util::DeviceExt;

use crate::{
    image_utils::image_from_color, model::Model, renderer::WgpuRenderer, texture::Texture,
};

// TODO
// this is temporary until Meshes have handles to their material and
// Models are just a list of Mesh handles
#[derive(Component)]
pub struct GpuModelMaterials {
    pub data: Vec<(
        MaterialUniform,
        wgpu::Buffer,
        wgpu::BindGroup,
        UniformBuffer<Vec<u8>>,
    )>,
}

#[derive(ShaderType)]
pub struct MaterialUniform {
    pub base_color: Vec4,
    pub alpha: f32,
    pub gloss: f32,
    pub specular: Vec3,
    pub flags: u32,
}

// WARN these must match the flags in shader.wgsl
bitflags::bitflags! {
    #[repr(transparent)]
    pub struct MaterialFlags: u32 {
        const USE_NORMAL_MAP = (1 << 0);
        const _1 = (1 << 1);
        const _2 = (1 << 2);
        const _3 = (1 << 3);
        const _4 = (1 << 4);
        const _5 = (1 << 5);
        const _6 = (1 << 6);
        const _7 = (1 << 7);
        const _8 = (1 << 8);
        const _9 = (1 << 9);
        const _10 = (1 << 10);
        const NONE = 0;
        const UNINITIALIZED = 0xFFFF;
    }
}

pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("material_bind_group_layout"),
        entries: &[
            // material
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // diffuse_texture
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // normal_texture
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            // specular_texture
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

pub fn create_material_uniform(
    mut commands: Commands,
    renderer: Res<WgpuRenderer>,
    query: Query<(Entity, &Model), (Added<Model>, Without<GpuModelMaterials>)>,
) {
    for (entity, model) in query.iter() {
        log::info!("New model detected");

        let mut gpu_materials = vec![];
        for material in &model.materials {
            let uniform = MaterialUniform {
                base_color: material.base_color,
                alpha: material.alpha,
                gloss: material.gloss,
                specular: material.specular,
                flags: if material.normal_texture.is_some() {
                    MaterialFlags::USE_NORMAL_MAP.bits()
                } else {
                    MaterialFlags::NONE.bits()
                },
            };

            let byte_buffer = Vec::new();
            let mut uniform_buffer = UniformBuffer::new(byte_buffer);
            uniform_buffer.write(&uniform).unwrap();

            let buffer = renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    contents: uniform_buffer.as_ref(),
                    label: None,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

            let diffuse_texture = Texture::from_image(
                &renderer.device,
                &renderer.queue,
                &material.diffuse_texture,
                Some(&format!("{}_diffuse_texture", material.name)),
                None,
            )
            .unwrap();

            let default_white = image_from_color(Color::WHITE);

            let normal_texture = Texture::from_image(
                &renderer.device,
                &renderer.queue,
                material.normal_texture.as_ref().unwrap_or(&default_white),
                Some(&format!("{}_normal_texture", material.name)),
                Some(wgpu::TextureFormat::Rgba8Unorm),
            )
            .unwrap();

            let specular_texture = Texture::from_image(
                &renderer.device,
                &renderer.queue,
                material.specular_texture.as_ref().unwrap_or(&default_white),
                Some(&format!("{}_specular_texture", material.name)),
                None,
            )
            .unwrap();

            let bind_group = renderer
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("{}_material_bind_group", material.name)),
                    layout: &bind_group_layout(&renderer.device),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        },
                        // diffuse
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                        },
                        // normal
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                        },
                        // specular
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::TextureView(&specular_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::Sampler(&specular_texture.sampler),
                        },
                    ],
                });
            gpu_materials.push((uniform, buffer, bind_group, uniform_buffer));
        }
        commands.entity(entity).insert(GpuModelMaterials {
            data: gpu_materials,
        });
    }
}

pub fn update_material_buffer(
    renderer: Res<WgpuRenderer>,
    mut query: Query<(&Model, &mut GpuModelMaterials), Changed<Model>>,
) {
    for (model, mut gpu_materials) in query.iter_mut() {
        for (i, mat) in model.materials.iter().enumerate() {
            let u = MaterialUniform {
                base_color: mat.base_color,
                alpha: mat.alpha,
                gloss: mat.gloss,
                specular: mat.specular,
                flags: if mat.normal_texture.is_some() {
                    MaterialFlags::USE_NORMAL_MAP.bits()
                } else {
                    MaterialFlags::NONE.bits()
                },
            };
            gpu_materials.data[i]
                .3
                .write(&u)
                .expect("failed to write to material buffer");
            // TODO I have no idea if this actually works since I don't change any material at runtime
            renderer.queue.write_buffer(
                &gpu_materials.data[i].1,
                0,
                gpu_materials.data[i].3.as_ref(),
            );
            gpu_materials.data[i].0 = u;
        }
    }
}
