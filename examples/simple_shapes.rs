use bevy::{input::InputPlugin, prelude::*, window::WindowPlugin, winit::WinitPlugin};

use glace::{
    camera::CameraSettings,
    egui_plugin::EguiPlugin,
    light::Light,
    model::{self, Model},
    renderer::{wireframe::Wireframe, GlaceClearColor, WgpuRenderer, WgpuRendererPlugin},
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
        .add_plugin(WinitPlugin)
        .add_plugin(InputPlugin::default())
        .add_plugin(WgpuRendererPlugin)
        .add_plugin(EguiPlugin)
        .add_startup_system(spawn_light)
        .add_startup_system(spawn_shapes)
        .run();
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

    commands.spawn().insert(light).insert(model);
}

fn spawn_shapes(mut commands: Commands, renderer: Res<WgpuRenderer>) {
    let diffuse_texture_bytes = include_bytes!("../assets/rock_plane/Rock-Albedo.png");
    let diffuse_texture = image::load_from_memory(diffuse_texture_bytes)
        .unwrap()
        .to_rgba8();

    let normal_texture_bytes = include_bytes!("../assets/rock_plane/Rock-Normal.png");
    let normal_texture = image::load_from_memory(normal_texture_bytes)
        .unwrap()
        .to_rgba8();

    let plane = Model {
        meshes: vec![shapes::plane::Plane {
            resolution: 5,
            size: 5.0,
        }
        .mesh(&renderer.device)],
        materials: vec![model::Material {
            name: "rock_material".to_string(),
            diffuse_texture,
            normal_texture: Some(normal_texture),
            ..Default::default()
        }],
    };
    commands.spawn_bundle((
        plane,
        Transform {
            translation: Vec3::new(-2.5, -1.0, -2.5),
            ..default()
        },
    ));

    let cube = Model {
        meshes: vec![shapes::cube::Cube::new(1.0, 1.0, 1.0).mesh(&renderer.device)],
        materials: vec![model::Material::from_color(Color::WHITE)],
    };
    commands.spawn_bundle((
        cube,
        Transform {
            translation: Vec3::ZERO - (Vec3::X * 1.5),
            ..default()
        },
    ));

    let sphere = Model {
        meshes: vec![shapes::sphere::UVSphere::default().mesh(&renderer.device)],
        materials: vec![model::Material::from_color(Color::WHITE)],
    };
    commands.spawn_bundle((
        sphere,
        Transform {
            translation: Vec3::ZERO,
            ..default()
        },
    ));

    let capsule = Model {
        meshes: vec![shapes::capsule::Capsule::default().mesh(&renderer.device)],
        materials: vec![model::Material::from_color(Color::WHITE)],
    };
    commands
        .spawn_bundle((
            capsule,
            Transform {
                translation: Vec3::ZERO + (Vec3::X * 1.5),
                ..default()
            },
        ))
        .insert(Wireframe);
}
