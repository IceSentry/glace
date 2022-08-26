use anyhow::Context;
use bevy::{asset::LoadContext, prelude::*, tasks::IoTaskPool};
use image::RgbaImage;
use std::io::{BufReader, Cursor};

use crate::{image_utils::image_from_color, mesh::Mesh, mesh::Vertex, model::Material};

use super::LoadedObj;

pub async fn load_obj<'a, 'b>(
    bytes: &'a [u8],
    load_context: &'a LoadContext<'b>,
) -> anyhow::Result<LoadedObj> {
    let (obj_models, obj_materials) = tobj::load_obj_buf_async(
        &mut BufReader::new(Cursor::new(bytes)),
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |mtl_path| async move {
            let path = load_context.path().parent().unwrap().join(mtl_path);
            let mtl_bytes = load_context.read_asset_bytes(&path).await.unwrap();
            let mtl = tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mtl_bytes)));
            log::info!("Finished loading {path:?}");
            mtl
        },
    )
    .await
    .with_context(|| format!("Failed to load obj {:?}", load_context.path()))?;

    let obj_materials = obj_materials?;
    let mut materials: Vec<Material> = IoTaskPool::get()
        .scope(|scope| {
            obj_materials.iter().for_each(|obj_material| {
                log::info!("Loading {}", obj_material.name);
                scope.spawn(async move { load_material(load_context, obj_material).await });
            });
        })
        .into_iter()
        .filter_map(|res| {
            if let Err(err) = res.as_ref() {
                log::error!("Error while loading obj materials: {err}");
            }
            log::info!("Finished loading material: {}", res.as_ref().unwrap().name);
            res.ok()
        })
        .collect();
    if materials.is_empty() {
        materials.push(Material::default())
    }

    let meshes = generate_mesh(&obj_models, &materials);

    Ok(LoadedObj { materials, meshes })
}

async fn load_material<'a>(
    load_context: &LoadContext<'a>,
    obj_material: &tobj::Material,
) -> anyhow::Result<Material> {
    let diffuse_texture = load_texture(load_context, &obj_material.diffuse_texture)
        .await?
        .unwrap_or_else(|| image_from_color(Color::WHITE));
    let normal_texture = load_texture(load_context, &obj_material.normal_texture).await?;
    let specular_texture = load_texture(load_context, &obj_material.specular_texture).await?;

    Ok(Material {
        name: obj_material.name.clone(),
        base_color: Vec3::from(obj_material.diffuse).extend(obj_material.dissolve),
        diffuse_texture,
        alpha: obj_material.dissolve,
        gloss: obj_material.shininess,
        specular: Vec3::from(obj_material.specular),
        normal_texture,
        specular_texture,
    })
}

async fn load_texture<'a>(
    load_context: &LoadContext<'a>,
    texture_path: &str,
) -> anyhow::Result<Option<RgbaImage>> {
    Ok(if !texture_path.is_empty() {
        let bytes = load_context
            .read_asset_bytes(load_context.path().parent().unwrap().join(&texture_path))
            .await?;
        log::info!("Finished loading texture: {texture_path:?}");
        let rgba = image::load_from_memory(&bytes)?.to_rgba8();
        Some(rgba)
    } else {
        None
    })
}

fn generate_mesh(obj_models: &[tobj::Model], materials: &[Material]) -> Vec<Mesh> {
    obj_models
        .iter()
        .map(|m| {
            let vertices: Vec<_> = (0..m.mesh.positions.len() / 3)
                .map(|i| Vertex {
                    position: Vec3::new(
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ),
                    uv: if m.mesh.texcoords.is_empty() {
                        Vec2::ZERO
                    } else {
                        // UVs are flipped
                        Vec2::new(m.mesh.texcoords[i * 2], 1.0 - m.mesh.texcoords[i * 2 + 1])
                    },
                    normal: if m.mesh.normals.is_empty() {
                        Vec3::ZERO
                    } else {
                        Vec3::new(
                            m.mesh.normals[i * 3],
                            m.mesh.normals[i * 3 + 1],
                            m.mesh.normals[i * 3 + 2],
                        )
                    },
                    tangent: Vec3::ZERO,
                    bitangent: Vec3::ZERO,
                })
                .collect();

            let mut mesh = crate::mesh::Mesh {
                vertices,
                indices: Some(m.mesh.indices.clone()),
                material_id: m.mesh.material_id,
            };

            if m.mesh.normals.is_empty() {
                mesh.compute_normals();
            }
            if !m.mesh.normals.is_empty()
                && m.mesh
                    .material_id
                    .and_then(|m_id| materials[m_id].normal_texture.clone())
                    .is_some()
            {
                mesh.compute_tangents();
            }

            mesh
        })
        .collect()
}
