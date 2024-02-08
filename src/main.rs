use bevy::{
    a11y::AccessibilityPlugin,
    input::InputPlugin,
    prelude::*,
    window::{PrimaryWindow, RawHandleWrapper, WindowResized},
    winit::{WinitPlugin, WinitWindows},
};
use wgpu::{Backends, Features, Limits};
use winit::dpi::PhysicalSize;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            WindowPlugin {
                primary_window: Some(Window {
                    title: "glace2".into(),
                    ..default()
                }),
                ..default()
            },
            AccessibilityPlugin,
            WinitPlugin::default(),
            InputPlugin,
        ))
        .add_systems(Startup, setup_renderer)
        .add_systems(Update, (resize, render))
        .run();
}

#[derive(Resource, Deref, DerefMut)]
struct Device(wgpu::Device);
#[derive(Resource, Deref, DerefMut)]
struct Queue(wgpu::Queue);
#[derive(Resource, Deref, DerefMut)]
struct SurfaceConfiguration(wgpu::SurfaceConfiguration);
#[derive(Resource, Deref, DerefMut)]
struct Surface(wgpu::Surface<'static>);

fn setup_renderer(
    mut commands: Commands,
    primary_window: Query<(Entity, &RawHandleWrapper), With<PrimaryWindow>>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    let (window_entity, raw_handle_wrapper) = primary_window.single();
    let winit_window = winit_windows
        .get_window(window_entity)
        .expect("Failed to get winit window");
    let size = winit_window.inner_size();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: Backends::all(),
        ..default()
    });
    let surface = instance
        .create_surface(unsafe { raw_handle_wrapper.get_handle() })
        .expect("Failed to create surface");

    let adapter =
        futures_lite::future::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to request adapter");

    let (device, queue) = futures_lite::future::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            required_features: Features::empty(),
            required_limits: Limits::default(),
            label: None,
        },
        None,
    ))
    .expect("Failed to request device");

    let config = surface
        .get_default_config(&adapter, size.width, size.height)
        .expect("Failed to get default surface config");
    surface.configure(&device, &config);

    commands.insert_resource(Device(device));
    commands.insert_resource(Queue(queue));
    commands.insert_resource(SurfaceConfiguration(config));
    commands.insert_resource(Surface(surface));
}

fn resize(
    mut surface_config: ResMut<SurfaceConfiguration>,
    surface: Res<Surface>,
    device: Res<Device>,
    mut events: EventReader<WindowResized>,
    windows: Query<&Window>,
) {
    for event in events.read() {
        let window = windows.get(event.window).expect("window not found");
        let width = window.physical_width();
        let height = window.physical_height();

        let new_size = PhysicalSize { width, height };

        if new_size.width > 0 && new_size.height > 0 {
            surface_config.width = new_size.width;
            surface_config.height = new_size.height;

            surface.configure(&device, &surface_config);
        }
    }
}

fn render(surface: Res<Surface>, device: Res<Device>, queue: Res<Queue>) {
    let output = surface
        .get_current_texture()
        .expect("Failed to get texture");
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut command_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    {
        let _render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
    }

    queue.submit(std::iter::once(command_encoder.finish()));
    output.present();
}
