use crate::{
    mesh::Mesh,
    model::{Material, Model, ModelMesh},
    obj_loader::loader::load_obj,
    renderer::WgpuRenderer,
};
use bevy::{
    asset::{AssetLoader, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::Instant,
};

mod loader;

// References:
// <https://andrewnoske.com/wiki/OBJ_file_format>
// <http://paulbourke.net/dataformats/mtl/>
// <https://en.wikipedia.org/wiki/Wavefront_.obj_file>

pub struct ObjLoaderPlugin;
impl Plugin for ObjLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<LoadedObj>()
            .init_asset_loader::<ObjLoader>()
            .add_system(obj_spawner);
    }
}

#[derive(Debug, TypeUuid)]
#[uuid = "39cadc56-aa9c-4543-8640-a018b74b5052"]
pub struct LoadedObj {
    pub materials: Vec<Material>,
    pub meshes: Vec<Mesh>,
}

#[derive(Default)]
pub struct ObjLoader;
impl AssetLoader for ObjLoader {
    fn extensions(&self) -> &[&str] {
        &["obj"]
    }

    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::asset::BoxedFuture<'a, anyhow::Result<(), anyhow::Error>> {
        Box::pin(async move {
            let start = Instant::now();

            log::info!("Loading {:?}", load_context.path());

            let obj = load_obj(bytes, load_context).await?;
            load_context.set_default_asset(LoadedAsset::new(obj));

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
pub struct ObjBundle {
    pub obj: Handle<LoadedObj>,
}

fn obj_spawner(
    mut commands: Commands,
    renderer: Res<WgpuRenderer>,
    query: Query<(Entity, &Handle<LoadedObj>), Without<Model>>,
    obj_assets: Res<Assets<LoadedObj>>,
) {
    for (entity, obj_handle) in query.iter() {
        if let Some(obj) = obj_assets.get(obj_handle) {
            let LoadedObj { materials, meshes } = obj;

            // TODO mesh label for obj
            let model_meshes = meshes
                .iter()
                .map(|mesh| ModelMesh::from_mesh("", &renderer.device, mesh))
                .collect();

            commands.entity(entity).insert(Model {
                materials: materials.clone(),
                meshes: model_meshes,
            });

            log::info!("Obj Model spawned");
        }
    }
}
