use bevy::{
    app::prelude::*,
    ecs::prelude::*,
    input::{mouse::MouseMotion, prelude::*},
    math::prelude::*,
    time::prelude::*,
    window::prelude::*,
};

use crate::renderer::bind_groups::mesh_view::CameraUniform;

const FRICTION: f32 = 0.5;

const CAMERRA_EYE: Vec3 = Vec3::from_array([0.0, 5.0, 8.0]);

#[derive(Resource)]
pub struct CameraSettings {
    pub speed: f32,
}

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, setup_camera)
            .add_systems(Update, fly_camera);
    }
}

pub struct Projection {
    pub aspect: f32,
    pub fov_y: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl Projection {
    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn compute_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far)
    }
}

#[derive(Resource)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub rotation: Quat,
    pub projection: Projection,
}

impl Camera {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            eye: CAMERRA_EYE,
            target: Vec3::ZERO,
            projection: Projection {
                aspect: width / height,
                fov_y: 45.0,
                z_near: 0.1,
                z_far: 1000.0,
            },
            rotation: Quat::from_mat4(&Mat4::look_at_rh(CAMERRA_EYE, Vec3::ZERO, Vec3::Y))
                .inverse(),
        }
    }

    pub fn build_view_projection_matrix(&self) -> Mat4 {
        let view = Mat4::from_rotation_translation(self.rotation, self.eye);
        let proj = self.projection.compute_matrix();
        proj * view.inverse()
    }

    #[inline]
    pub fn forward(&self) -> Vec3 {
        -self.local_z()
    }

    #[inline]
    pub fn right(&self) -> Vec3 {
        self.local_x()
    }
    #[allow(unused)]
    #[inline]
    pub fn up(&self) -> Vec3 {
        self.local_y()
    }

    #[inline]
    pub fn local_x(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    #[allow(unused)]
    #[inline]
    pub fn local_y(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    #[inline]
    pub fn local_z(&self) -> Vec3 {
        self.rotation * Vec3::Z
    }
}

fn setup_camera(mut commands: Commands, windows: Query<&Window>) {
    let window = windows.single();
    let camera = Camera::new(window.width(), window.height());

    let mut camera_uniform = CameraUniform::new();
    camera_uniform.update_view_proj(&camera);

    commands.insert_resource(camera);
    commands.insert_resource(camera_uniform);
}

fn fly_camera(
    time: Res<Time>,
    windows: Query<&Window>,
    mouse_input: Res<Input<MouseButton>>,
    key_input: Res<Input<KeyCode>>,
    mut camera: ResMut<Camera>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut velocity: Local<Vec3>,
    settings: Res<CameraSettings>,
) {
    if !mouse_input.pressed(MouseButton::Right) {
        return;
    }

    let dt = time.delta_seconds();

    // Rotate

    let mut mouse_delta = Vec2::ZERO;
    for mouse_motion in mouse_motion.iter() {
        mouse_delta += mouse_motion.delta;
    }

    if mouse_delta != Vec2::ZERO {
        let window = if let Ok(window) = windows.get_single() {
            Vec2::new(window.width(), window.height())
        } else {
            Vec2::ZERO
        };
        let delta_x = mouse_delta.x / window.x * std::f32::consts::TAU;
        let delta_y = mouse_delta.y / window.y * std::f32::consts::PI;
        let yaw = Quat::from_rotation_y(-delta_x);
        let pitch = Quat::from_rotation_x(-delta_y);
        camera.rotation = yaw * camera.rotation; // rotate around global y axis
        camera.rotation *= pitch; // rotate around local x axis
    }

    // Translate

    let mut axis_input = Vec3::ZERO;
    if key_input.pressed(KeyCode::W) {
        axis_input.z += 1.0;
    }
    if key_input.pressed(KeyCode::S) {
        axis_input.z -= 1.0;
    }
    if key_input.pressed(KeyCode::D) {
        axis_input.x += 1.0;
    }
    if key_input.pressed(KeyCode::A) {
        axis_input.x -= 1.0;
    }
    if key_input.pressed(KeyCode::Space) {
        axis_input.y += 1.0;
    }
    if key_input.pressed(KeyCode::ShiftLeft) {
        axis_input.y -= 1.0;
    }

    if axis_input != Vec3::ZERO {
        *velocity = axis_input.normalize() * settings.speed;
    } else {
        *velocity *= 1.0 - FRICTION;
        if velocity.length_squared() < 1e-6 {
            *velocity = Vec3::ZERO;
        }
    }

    let forward = camera.forward();
    let right = camera.right();
    camera.eye += velocity.x * dt * right + velocity.y * dt * Vec3::Y + velocity.z * dt * forward;
}
