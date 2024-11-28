use std::borrow::Cow;

use bevy::{
    a11y::AccessibilityPlugin,
    input::InputPlugin,
    prelude::*,
    window::{PresentMode, PrimaryWindow, RawHandleWrapper, WindowResized},
    winit::{WakeUp, WinitPlugin, WinitWindows},
};
use wgpu::{Backends, Features, Limits, MemoryHints};
use winit::dpi::PhysicalSize;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            WindowPlugin {
                primary_window: Some(Window {
                    title: "glace2".into(),
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            },
            AccessibilityPlugin,
            WinitPlugin::<WakeUp>::default(),
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
#[derive(Resource, Deref, DerefMut)]
struct RenderPipeline(wgpu::RenderPipeline);

fn setup_renderer(
    mut commands: Commands,
    primary_window: Query<(Entity, &Window, &RawHandleWrapper), With<PrimaryWindow>>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    let (window_entity, window, raw_handle_wrapper) = primary_window.single();
    let winit_window = winit_windows
        .get_window(window_entity)
        .expect("Failed to get winit window");
    let mut size = winit_window.inner_size();
    size.width = size.width.max(1);
    size.height = size.height.max(1);

    let instance = wgpu::Instance::default();
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
            required_limits: Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            label: Some("RenderDevice"),
            memory_hints: MemoryHints::MemoryUsage,
        },
        None,
    ))
    .expect("Failed to request device");

    let mut config = surface
        .get_default_config(&adapter, size.width, size.height)
        .expect("Failed to get default surface config");
    config.present_mode = match window.present_mode {
        PresentMode::AutoVsync => wgpu::PresentMode::AutoVsync,
        PresentMode::AutoNoVsync => wgpu::PresentMode::AutoNoVsync,
        PresentMode::Fifo => wgpu::PresentMode::Fifo,
        PresentMode::FifoRelaxed => wgpu::PresentMode::FifoRelaxed,
        PresentMode::Immediate => wgpu::PresentMode::Immediate,
        PresentMode::Mailbox => wgpu::PresentMode::Mailbox,
    };
    surface.configure(&device, &config);

    let swapchain_capabilities = surface.get_capabilities(&adapter);
    let swapchain_format = swapchain_capabilities.formats[0];

    // Load the shaders from disk
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(swapchain_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    println!("Renderer setup done!");

    commands.insert_resource(Device(device));
    commands.insert_resource(Queue(queue));
    commands.insert_resource(SurfaceConfiguration(config));
    commands.insert_resource(Surface(surface));
    commands.insert_resource(RenderPipeline(render_pipeline));
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

fn render(
    surface: Res<Surface>,
    device: Res<Device>,
    queue: Res<Queue>,
    render_pipeline: Res<RenderPipeline>,
) {
    println!("render");
    let frame = surface
        .get_current_texture()
        .expect("Failed to get texture");
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut command_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
    });

    {
        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
        rpass.set_pipeline(&render_pipeline);
        rpass.draw(0..3, 0..1);
    }

    queue.submit(Some(command_encoder.finish()));
    frame.present();
    println!("present done.");
}
