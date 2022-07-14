use bevy::{
    asset::AssetPlugin, input::InputPlugin, prelude::*, window::WindowPlugin, winit::WinitPlugin,
};

use glace::{
    camera::CameraSettings,
    egui_plugin::EguiPlugin,
    instances::Instances,
    light::Light,
    model::Model,
    obj_loader::{ObjBundle, ObjLoaderPlugin},
    renderer::{
        plugin::WgpuRendererPlugin, render_phase_3d::RenderPhase3dDescriptor, WgpuRenderer,
    },
    shapes,
};

const LIGHT_POSITION: Vec3 = Vec3::from_array([4.0, 4.0, 2.0]);

const NUM_INSTANCES_PER_ROW: u32 = 6;
const SPACE_BETWEEN: f32 = 3.0;

// const MODEL_NAME: &str = "models/obj/large_obj/sponza_obj/sponza.obj";
// const MODEL_NAME: &str = "models/obj/large_obj/bistro/Exterior/exterior.obj";
// const SCALE: Vec3 = Vec3::from_array([0.05, 0.05, 0.05]);

const MODEL_NAME: &str = "models/obj/teapot/teapot.obj";
const SCALE: Vec3 = Vec3::from_array([0.025, 0.025, 0.025]);

// const MODEL_NAME: &str = "models/obj/cube/cube.obj";
// const MODEL_NAME: &str = "models/obj/learn_opengl/container2/cube.obj";
// const SCALE: Vec3 = Vec3::from_array([1.0, 1.0, 1.0]);

// const MODEL_NAME: &str = "models/obj/bunny.obj";
// const SCALE: Vec3 = Vec3::from_array([1.5, 1.5, 1.5]);

const INSTANCED_MODEL_NAME: &str = "models/obj/cube/cube.obj";
const INSTANCED_SCALE: Vec3 = Vec3::from_array([1.0, 1.0, 1.0]);

struct InstanceSettings {
    move_instances: bool,
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .init();

    App::new()
        .insert_resource(RenderPhase3dDescriptor {
            clear_color: Color::rgba(0.1, 0.1, 0.1, 1.0),
            ..default()
        })
        .insert_resource(CameraSettings { speed: 10.0 })
        .insert_resource(InstanceSettings {
            move_instances: false,
        })
        .add_plugins(MinimalPlugins)
        .add_plugin(WindowPlugin::default())
        .add_plugin(WinitPlugin)
        .add_plugin(InputPlugin::default())
        .add_plugin(AssetPlugin)
        .add_plugin(WgpuRendererPlugin)
        .add_plugin(EguiPlugin)
        .add_plugin(ObjLoaderPlugin)
        .add_startup_system(spawn_obj)
        .add_startup_system(spawn_light)
        .add_system(update_light)
        .add_system(settings_ui)
        .add_system(move_instances)
        .run();
}

fn spawn_obj(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut instances = Vec::new();
    for z in 0..=NUM_INSTANCES_PER_ROW {
        for x in 0..=NUM_INSTANCES_PER_ROW {
            let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
            let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

            let translation = Vec3::new(x as f32, 0.0, z as f32);
            let rotation = if translation == Vec3::ZERO {
                Quat::from_axis_angle(Vec3::Y, 0.0)
            } else {
                Quat::from_axis_angle(translation.normalize(), std::f32::consts::FRAC_PI_4)
            };

            instances.push(Transform {
                rotation,
                translation,
                scale: INSTANCED_SCALE,
            });
        }
    }

    commands
        .spawn_bundle(ObjBundle {
            obj: asset_server.load(INSTANCED_MODEL_NAME),
        })
        .insert(Instances(instances))
        .insert(Wave::default());

    commands
        .spawn_bundle(ObjBundle {
            obj: asset_server.load(MODEL_NAME),
        })
        .insert(Transform {
            scale: SCALE,
            translation: Vec3::new(0.0, 2.0, 0.0),
            ..default()
        });
}

fn move_instances(
    time: Res<Time>,
    mut query: Query<(&mut Instances, &mut Wave)>,
    settings: Res<InstanceSettings>,
) {
    if !settings.move_instances {
        return;
    }
    for (mut instances, mut wave) in query.iter_mut() {
        wave.offset += time.delta_seconds() * wave.frequency;
        for instance in instances.0.iter_mut() {
            instance.translation.y =
                wave.wave_height(instance.translation.x, instance.translation.z);
        }
    }
}

#[derive(Component)]
pub struct Wave {
    pub amplitude: f32,
    pub wavelength: f32,
    pub frequency: f32,
    pub offset: f32,
}

impl Default for Wave {
    fn default() -> Self {
        Self {
            amplitude: 1.0,
            wavelength: 10.0,
            frequency: 2.0,
            offset: 0.0,
        }
    }
}

impl Wave {
    pub fn wave_height(&self, x: f32, z: f32) -> f32 {
        // Wave number
        let k = std::f32::consts::TAU / self.wavelength;
        let r = (x * x + z * z).sqrt();
        self.amplitude * (k * (r - self.offset)).sin()
    }
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

fn settings_ui(
    ctx: Res<egui::Context>,
    mut camera_settings: ResMut<CameraSettings>,
    mut instance_settings: ResMut<InstanceSettings>,
) {
    egui::Window::new("Settings")
        .resizable(true)
        .collapsible(true)
        .show(&ctx, |ui| {
            ui.heading("Camera");

            ui.label("Speed");
            ui.add(egui::Slider::new(&mut camera_settings.speed, 1.0..=20.0).step_by(0.5));

            ui.separator();

            ui.heading("Instances");

            ui.checkbox(&mut instance_settings.move_instances, "Move");
        });
}
