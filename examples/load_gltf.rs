use bevy::{
    a11y::AccessibilityPlugin, asset::AssetPlugin, input::InputPlugin, prelude::*,
    window::WindowPlugin, winit::WinitPlugin,
};

use glace::{
    camera::CameraSettings,
    egui_plugin::EguiPlugin,
    gltf_loader::{GltfBundle, GltfLoaderPlugin},
    light::Light,
    model::Model,
    renderer::{GlaceClearColor, WgpuRenderer, WgpuRendererPlugin},
    shapes,
};

const LIGHT_POSITION: Vec3 = Vec3::from_array([2.0, 2.0, 2.0]);

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .init();

    App::new()
        .insert_resource(GlaceClearColor(Color::rgba(0.1, 0.1, 0.1, 1.0)))
        .insert_resource(CameraSettings { speed: 10.0 })
        .add_plugins(MinimalPlugins)
        .add_plugin(WindowPlugin::default())
        .add_plugin(AccessibilityPlugin)
        .add_plugin(WinitPlugin)
        .add_plugin(InputPlugin::default())
        .add_plugin(AssetPlugin::default())
        .add_plugin(WgpuRendererPlugin)
        .add_plugin(EguiPlugin)
        .add_plugin(GltfLoaderPlugin)
        .add_startup_system(spawn_gltf)
        .add_startup_system(spawn_light)
        .add_system(update_light)
        .run();
}

fn spawn_gltf(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn(GltfBundle {
            gltf: asset_server.load("models/gltf/FlightHelmet/FlightHelmet.gltf"),
        })
        .insert(Transform {
            scale: Vec3::new(2.5, 2.5, 2.5),
            ..default()
        });
}

fn spawn_light(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let cube = shapes::cube::Cube::new(1.0, 1.0, 1.0);
    let mesh = cube.mesh(&renderer.device);
    let model = Model {
        meshes: vec![mesh],
        materials: vec![],
    };

    let light = Light {
        position: LIGHT_POSITION,
        color: Color::WHITE.as_rgba_f32().into(),
    };

    commands.spawn((light, model));
}

fn update_light(mut query: Query<&mut Light>, time: Res<Time>) {
    let speed = 0.25;
    for mut light in query.iter_mut() {
        let old_position = light.position;
        light.position = Quat::from_axis_angle(
            Vec3::Y,
            std::f32::consts::TAU * time.delta_seconds() * speed,
        )
        .mul_vec3(old_position);
    }
}
