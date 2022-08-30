#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use crate::{
    camera::CameraSettings,
    egui_plugin::{EguiCtxRes, EguiPlugin},
    gltf_loader::{GltfBundle, GltfLoaderPlugin},
    light::Light,
    model::Model,
    obj_loader::{ObjBundle, ObjLoaderPlugin},
    renderer::{
        plugin::WgpuRendererPlugin, render_phase_3d::RenderPhase3dDescriptor, WgpuRenderer,
    },
};
use bevy::{
    app::AppExit,
    asset::AssetPlugin,
    input::{Input, InputPlugin},
    math::{Quat, Vec3},
    prelude::*,
    window::{CursorMoved, WindowDescriptor, WindowPlugin, Windows},
    winit::WinitPlugin,
    MinimalPlugins,
};

mod camera;
mod egui_plugin;
mod gltf_loader;
mod image_utils;
mod instances;
mod light;
mod mesh;
mod model;
mod obj_loader;
mod renderer;
mod shapes;
mod texture;
mod transform;

const LIGHT_POSITION: Vec3 = Vec3::from_array([4.0, 4.0, 0.0]);

#[derive(Resource)]
struct LightSettings {
    rotate: bool,
    color: [f32; 3],
    speed: f32,
}

#[derive(Resource)]
struct GlobalMaterialSettings {
    gloss: f32,
}

#[derive(Resource)]
struct InstanceSettings {
    move_instances: bool,
}

#[derive(Resource)]
struct ModelSettings {
    scale: f32,
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .init();

    App::new()
        .insert_resource(WindowDescriptor {
            // width: 800.0,
            // height: 600.0,
            // mode: WindowMode::Fullscreen,
            ..default()
        })
        .insert_resource(RenderPhase3dDescriptor {
            clear_color: Color::rgba(0.0, 0.0, 0.0, 1.0),
            ..default()
        })
        .insert_resource(CameraSettings { speed: 10.0 })
        .insert_resource(LightSettings {
            rotate: true,
            color: [1.0, 1.0, 1.0],
            speed: 0.35,
        })
        .insert_resource(GlobalMaterialSettings { gloss: 0.5 })
        .insert_resource(InstanceSettings {
            move_instances: false,
        })
        .insert_resource(ModelSettings { scale: 1.0 })
        .add_plugins(MinimalPlugins)
        .add_plugin(WindowPlugin::default())
        .add_plugin(WinitPlugin)
        .add_plugin(InputPlugin::default())
        .add_plugin(WgpuRendererPlugin)
        .add_plugin(AssetPlugin)
        .add_plugin(EguiPlugin)
        .add_plugin(ObjLoaderPlugin)
        .add_plugin(GltfLoaderPlugin)
        .add_startup_system(spawn_light)
        .add_system(update_window_title)
        .add_system(update_show_depth)
        // .add_system(cursor_moved)
        .add_system(update_light)
        .add_system(exit_on_esc)
        .add_system(settings_ui)
        .add_system(update_materials)
        .add_system(update_model)
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

fn update_window_title(time: Res<Time>, mut windows: ResMut<Windows>) {
    if let Some(window) = windows.get_primary_mut() {
        window.set_title(format!("dt: {}ms", time.delta().as_millis()));
    }
}

fn update_show_depth(
    keyboard_input: Res<Input<KeyCode>>,
    mut descriptor: ResMut<RenderPhase3dDescriptor>,
) {
    if keyboard_input.just_pressed(KeyCode::X) {
        descriptor.show_depth_buffer = !descriptor.show_depth_buffer;
    }
}

#[allow(unused)]
fn cursor_moved(
    renderer: Res<WgpuRenderer>,
    mut events: EventReader<CursorMoved>,
    mut descriptor: ResMut<RenderPhase3dDescriptor>,
) {
    for event in events.iter() {
        descriptor.clear_color = Color::rgb(
            event.position.x as f32 / renderer.size.width as f32,
            event.position.y as f32 / renderer.size.height as f32,
            descriptor.clear_color.b(),
        );
    }
}

fn exit_on_esc(key_input: Res<Input<KeyCode>>, mut exit_events: EventWriter<AppExit>) {
    if key_input.just_pressed(KeyCode::Escape) {
        exit_events.send_default();
    }
}

fn update_light(mut query: Query<&mut Light>, time: Res<Time>, settings: Res<LightSettings>) {
    if !settings.rotate {
        return;
    }
    for mut light in query.iter_mut() {
        let old_position = light.position;
        light.position = Quat::from_axis_angle(
            Vec3::Y,
            std::f32::consts::TAU * time.delta_seconds() * settings.speed,
        )
        .mul_vec3(old_position);
        light.color = settings.color.into();
    }
}

fn update_materials(mut query: Query<&mut Model>, settings: Res<GlobalMaterialSettings>) {
    if !settings.is_changed() {
        return;
    }

    for mut model in query.iter_mut() {
        for mut material in model.materials.iter_mut() {
            material.gloss = settings.gloss;
        }
    }
}

fn update_model(mut query: Query<&mut Transform, With<Model>>, settings: Res<ModelSettings>) {
    if !settings.is_changed() {
        return;
    }
    for mut transform in &mut query {
        transform.scale = Vec3::ONE * settings.scale;
    }
}

fn settings_ui(
    mut commands: Commands,
    ctx: Res<EguiCtxRes>,
    asset_server: Res<AssetServer>,
    mut camera_settings: ResMut<CameraSettings>,
    mut light_settings: ResMut<LightSettings>,
    mut global_material_settings: ResMut<GlobalMaterialSettings>,
    mut instance_settings: ResMut<InstanceSettings>,
    mut model_settings: ResMut<ModelSettings>,
    mut descriptor: ResMut<RenderPhase3dDescriptor>,
    mut spawned_entity: Local<Option<Entity>>,
) {
    egui::Window::new("Settings")
        .resizable(true)
        .collapsible(true)
        .show(&ctx.0, |ui| {
            ui.heading("Camera");
            ui.label("Speed");
            ui.add(egui::Slider::new(&mut camera_settings.speed, 1.0..=20.0).step_by(0.5));

            ui.separator();

            ui.heading("Light");
            ui.checkbox(&mut light_settings.rotate, "Rotate");
            ui.label("Speed");
            ui.add(egui::Slider::new(&mut light_settings.speed, 0.0..=2.0).step_by(0.05));
            ui.label("Color");
            ui.color_edit_button_rgb(&mut light_settings.color);

            ui.separator();

            ui.heading("Global Material");
            ui.label("Gloss");
            ui.add(egui::Slider::new(
                &mut global_material_settings.gloss,
                0.0..=1.0,
            ));

            ui.separator();

            ui.heading("Instances");
            ui.checkbox(&mut instance_settings.move_instances, "Move");

            ui.separator();

            ui.heading("Model");
            ui.label("scale");
            ui.add(egui::Slider::new(&mut model_settings.scale, 0.025..=5.0));

            ui.separator();

            ui.heading("shader");
            ui.checkbox(&mut descriptor.show_depth_buffer, "show depth buffer");
        });

    egui::Window::new("Spawner")
        .resizable(true)
        .collapsible(true)
        .show(&ctx.0, |ui| {
            ui.heading("obj");

            let mut spawn_obj = |model_name: &str| {
                if let Some(spawned_entity) = *spawned_entity {
                    commands.entity(spawned_entity).despawn_recursive();
                }
                let entity = commands
                    .spawn_bundle(ObjBundle {
                        obj: asset_server.load(&format!("models/obj/{model_name}")),
                    })
                    .insert(Transform::default())
                    .id();
                *spawned_entity = Some(entity);
            };
            if ui.button("spawn sponza").clicked() {
                model_settings.scale = 0.025;
                spawn_obj("large_obj/sponza/sponza.obj");
            }
            if ui.button("spawn bistro").clicked() {
                model_settings.scale = 0.05;
                spawn_obj("large_obj/bistro/Exterior/exterior.obj");
            }
            if ui.button("spawn cube").clicked() {
                model_settings.scale = 1.0;
                spawn_obj("cube/cube.obj");
            }
            if ui.button("spawn cube2").clicked() {
                model_settings.scale = 1.0;
                spawn_obj("learn_opengl/container2/cube.obj");
            }
            if ui.button("spawn teapot").clicked() {
                model_settings.scale = 0.025;
                spawn_obj("teapot/teapot.obj");
            }
            if ui.button("spawn bunny").clicked() {
                model_settings.scale = 1.5;
                spawn_obj("bunny.obj");
            }

            ui.heading("GLTF");

            let mut spawn_gltf = |model_name: &str| {
                if let Some(spawned_entity) = *spawned_entity {
                    commands.entity(spawned_entity).despawn_recursive();
                }
                let entity = commands
                    .spawn_bundle(GltfBundle {
                        gltf: asset_server.load(&format!("models/gltf/{model_name}")),
                    })
                    .insert(Transform::default())
                    .id();
                *spawned_entity = Some(entity);
            };

            if ui.button("spawn sponza").clicked() {
                model_settings.scale = 0.025;
                spawn_gltf("sponza/Sponza.gltf");
            }
            if ui.button("spawn new sponza").clicked() {
                model_settings.scale = 1.0;
                spawn_gltf("/new-sponza/NewSponza_Main_Blender_glTF.gltf");
            }
            if ui.button("spawn bistro exterior").clicked() {
                model_settings.scale = 0.025;
                spawn_gltf("bistro/exterior/bistro_exterior.gltf");
            }
            if ui.button("spawn flight helmet").clicked() {
                model_settings.scale = 5.0;
                spawn_gltf("FlightHelmet/FlightHelmet.gltf");
            }
            if ui.button("spawn suzanne").clicked() {
                model_settings.scale = 1.0;
                spawn_gltf("suzanne/Suzanne.gltf");
            }
            if ui.button("spawn cube").clicked() {
                model_settings.scale = 1.0;
                spawn_gltf("learnopengl_cube/cube.gltf");
            }
        });
}
