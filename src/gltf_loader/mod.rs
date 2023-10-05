use crate::{
    gltf_loader::loader::load_gltf,
    model::{Material, Model, ModelMesh},
    renderer::WgpuRenderer,
};
use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::{TypePath, TypeUuid},
    utils::Instant,
};

mod loader;

pub struct GltfLoaderPlugin;
impl Plugin for GltfLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<LoadedGltf>()
            .init_asset_loader::<GltfLoader>()
            .add_systems(Update, gltf_spawner);
        // TODO improve loaded detection
        // .add_system(handle_loaded)
        // .add_system(handle_instanced_loaded);
    }
}

#[derive(Debug, TypeUuid, TypePath)]
#[uuid = "d87cb7a6-21b0-4c5a-933e-9edfe42e653b"]
pub struct LoadedGltf {
    materials: Vec<Material>,
    meshes: Vec<crate::mesh::Mesh>,
}
#[derive(Default)]
pub struct GltfLoader;
impl AssetLoader for GltfLoader {
    fn extensions(&self) -> &[&str] {
        &["gltf"]
    }

    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> bevy::asset::BoxedFuture<'a, anyhow::Result<(), anyhow::Error>> {
        Box::pin(async move {
            let start = Instant::now();

            log::info!("Loading {:?}", load_context.path());

            let loaded_gltf = load_gltf(bytes, load_context).await?;
            load_context.set_default_asset(LoadedAsset::new(loaded_gltf));

            log::info!(
                "Finished loading {:?} {}ms",
                load_context.path(),
                (Instant::now() - start).as_millis(),
            );

            Ok(())
        })
    }
}

#[derive(Default, Bundle)]
pub struct GltfBundle {
    pub gltf: Handle<LoadedGltf>,
}

fn gltf_spawner(
    mut commands: Commands,
    renderer: Res<WgpuRenderer>,
    query: Query<(Entity, &Handle<LoadedGltf>), Without<Model>>,
    gltf_assets: Res<Assets<LoadedGltf>>,
) {
    for (entity, gltf_handle) in query.iter() {
        if let Some(gltf) = gltf_assets.get(gltf_handle) {
            let LoadedGltf { materials, meshes } = gltf;

            // TODO mesh label for gltf
            let model_meshes = meshes
                .iter()
                .map(|mesh| ModelMesh::from_mesh("", &renderer.device, mesh))
                .collect();

            commands.entity(entity).insert(Model {
                materials: materials.clone(),
                meshes: model_meshes,
            });

            log::info!("Gltf Model spawned");
        }
    }
}
