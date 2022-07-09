use bevy::{
    asset::LoadContext,
    prelude::*,
    tasks::IoTaskPool,
    utils::{HashMap, Instant},
};
use image::RgbaImage;

use crate::{image_utils::image_from_color, mesh::Vertex, model::Material};

use super::LoadedGltf;

pub async fn load_gltf<'a, 'b>(
    bytes: &'a [u8],
    load_context: &'a mut LoadContext<'b>,
) -> anyhow::Result<LoadedGltf> {
    let gltf = gltf::Gltf::from_slice(bytes)?;

    let start = Instant::now();
    let textures = load_textures(&gltf, load_context);

    log::info!(
        "Loaded all textures in {}ms",
        (Instant::now() - start).as_millis()
    );

    let start = Instant::now();
    let materials = load_materials(&gltf, textures);
    log::info!(
        "Loaded all materials in {}ms",
        (Instant::now() - start).as_millis()
    );

    let buffer_data = load_buffers(&gltf, load_context).await?;

    let mut meshes = vec![];
    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            meshes.push(generate_mesh(primitive, &buffer_data)?);
        }
    }

    Ok(LoadedGltf { materials, meshes })
}

fn load_textures<'a>(
    gltf: &gltf::Gltf,
    load_context: &LoadContext<'a>,
) -> HashMap<usize, RgbaImage> {
    IoTaskPool::get()
        .scope(|scope| {
            gltf.textures().for_each(|gltf_texture| {
                let load_context: &LoadContext = load_context;
                scope.spawn(async move {
                    let texture_image = load_texture(&gltf_texture, load_context).await;
                    log::info!("loading {:?} completed", gltf_texture.name());
                    (gltf_texture.index(), texture_image)
                });
            });
        })
        .into_iter()
        .filter_map(|(index, res)| {
            if let Err(err) = res.as_ref() {
                log::error!("Error loading glTF texture: {err}");
            }
            res.ok().map(|res| (index, res))
        })
        .collect()
}

fn load_materials(gltf: &gltf::Gltf, textures: HashMap<usize, RgbaImage>) -> Vec<Material> {
    let mut materials = vec![];
    for material in gltf.materials() {
        log::info!("loading material: {:?}", material.name());
        let base_color_texture =
            if let Some(info) = material.pbr_metallic_roughness().base_color_texture() {
                // TODO this should use an asset handle instead
                textures[&info.texture().index()].clone()
            } else {
                image_from_color(Color::WHITE)
            };
        // let base_color_texture = image_from_color(Color::CYAN);
        let base_color = material.pbr_metallic_roughness().base_color_factor();
        let metallic = material.pbr_metallic_roughness().metallic_factor();
        let metallic_roughness_texture = material
            .pbr_metallic_roughness()
            .metallic_roughness_texture()
            .map(|info| textures[&info.texture().index()].clone());
        let normal_texture = material
            .normal_texture()
            .map(|texture| textures[&texture.texture().index()].clone());

        materials.push(Material {
            name: material
                .name()
                .unwrap_or("Unknown material name")
                .to_string(),
            base_color: Vec4::from(base_color),
            diffuse_texture: base_color_texture,
            alpha: match material.alpha_mode() {
                gltf::material::AlphaMode::Opaque => 1.0,
                gltf::material::AlphaMode::Mask | gltf::material::AlphaMode::Blend => 0.5,
            },
            gloss: metallic,
            specular_texture: metallic_roughness_texture,
            specular: Vec3::new(1.0, 1.0, 1.0),
            normal_texture,
        });
    }
    materials
}

fn generate_mesh(
    primitive: gltf::Primitive,
    buffer_data: &[Vec<u8>],
) -> anyhow::Result<crate::mesh::Mesh> {
    let _primitive_topology = match primitive.mode() {
        gltf::mesh::Mode::Triangles => wgpu::PrimitiveTopology::TriangleList,
        _ => anyhow::bail!("Only triangle list are currently supported"),
    };

    let reader = primitive.reader(|buffer| Some(&buffer_data[buffer.index()]));

    let positions = if let Some(positions) = reader.read_positions() {
        positions.map(Vec3::from).collect::<Vec<_>>()
    } else {
        anyhow::bail!("positions are required");
    };

    let normals = reader
        .read_normals()
        .map(|normals| normals.map(Vec3::from).collect::<Vec<_>>())
        .unwrap_or_default();

    let uvs = reader
        .read_tex_coords(0)
        .map(|uvs| uvs.into_f32().map(Vec2::from).collect::<Vec<_>>())
        .unwrap_or_default();

    let indices: Option<Vec<_>> = reader
        .read_indices()
        .map(|indices| indices.into_u32().collect());

    let vertices: Vec<_> = (0..positions.len())
        .map(|i| Vertex {
            position: positions[i],
            normal: if normals.is_empty() {
                Vec3::ZERO
            } else {
                normals[i]
            },
            uv: if uvs.is_empty() { Vec2::ZERO } else { uvs[i] },
            tangent: Vec3::ZERO,
            bitangent: Vec3::ZERO,
        })
        .collect();

    let mut mesh = crate::mesh::Mesh {
        vertices,
        indices,
        material_id: primitive.material().index(),
    };

    if normals.is_empty() {
        mesh.compute_normals();
    }

    // TODO should use tangents if present instead of computing it
    if !normals.is_empty() && primitive.material().normal_texture().is_some() {
        mesh.compute_tangents();
    }

    Ok(mesh)
}

/// Loads raw glTF buffers data
async fn load_buffers<'a>(
    gltf: &gltf::Gltf,
    load_context: &LoadContext<'a>,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut buffer_data = Vec::new();
    for buffer in gltf.buffers() {
        match buffer.source() {
            gltf::buffer::Source::Bin => {
                if let Some(blob) = gltf.blob.as_deref() {
                    buffer_data.push(blob.into());
                } else {
                    anyhow::bail!("Missing blob in gltf bin from {:?}", load_context.path());
                }
            }
            gltf::buffer::Source::Uri(uri) => {
                if uri.starts_with("data:") {
                    anyhow::bail!("data uri not supported {uri:?}");
                }

                let bytes = load_context
                    .read_asset_bytes(load_context.path().parent().unwrap().join(uri))
                    .await?;

                buffer_data.push(bytes);
            }
        }
    }
    Ok(buffer_data)
}

async fn load_texture<'a>(
    gltf_texture: &gltf::Texture<'a>,
    load_context: &LoadContext<'a>,
) -> anyhow::Result<RgbaImage> {
    let source = gltf_texture.source().source();
    Ok(match source {
        gltf::image::Source::View { .. } => todo!("Gltf view not supported"),
        gltf::image::Source::Uri { uri, mime_type } => {
            let image_path = load_context.path().parent().unwrap().join(uri);
            log::info!("uri: {uri} mime: {mime_type:?} path: {image_path:?}");
            let bytes = load_context.read_asset_bytes(image_path).await?;
            image::load_from_memory(&bytes)?.to_rgba8()
        }
    })
}
