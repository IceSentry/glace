use std::borrow::Cow;

use bevy::{
    a11y::AccessibilityPlugin,
    core::FrameCount,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::InputPlugin,
    log::LogPlugin,
    prelude::*,
    render::render_resource::{
        binding_types::texture_storage_2d, BindGroupEntries, BindGroupLayoutEntries,
    },
    window::{PresentMode, PrimaryWindow, RawHandleWrapper, WindowResized},
    winit::{WakeUp, WinitPlugin, WinitWindows},
};
use wgpu::{
    BindingResource, CommandEncoderDescriptor, Features, Limits, MemoryHints, ShaderStages,
    StoreOp, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use winit::dpi::PhysicalSize;

const MAIN_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            WindowPlugin {
                primary_window: Some(Window {
                    title: "glace2".into(),
                    present_mode: PresentMode::AutoVsync,
                    visible: false,
                    ..default()
                }),
                ..default()
            },
            AccessibilityPlugin,
            WinitPlugin::<WakeUp>::default(),
            FrameTimeDiagnosticsPlugin,
            InputPlugin,
            LogPlugin::default(),
        ))
        .add_systems(Startup, setup_renderer)
        .add_systems(Update, (resize, render).chain())
        .add_systems(Update, (quit_on_q, update_window_title))
        .run();
}

fn quit_on_q(input: Res<ButtonInput<KeyCode>>, mut exit_event: EventWriter<AppExit>) {
    if input.just_pressed(KeyCode::KeyQ) {
        exit_event.send_default();
    }
}

fn update_window_title(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    diagnostics: Res<DiagnosticsStore>,
) {
    for mut window in &mut windows {
        if let (Some(fps), Some(dt)) = (
            diagnostics
                .get(&FrameTimeDiagnosticsPlugin::FPS)
                .and_then(|fps| fps.smoothed()),
            diagnostics
                .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
                .and_then(|dt| dt.smoothed()),
        ) {
            window.title = format!("FPS: {:.0}, dt: {:.2}ms", fps, dt);
        }
    }
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
struct TrianglePipeline(wgpu::RenderPipeline);

#[derive(Resource)]
struct BlitPipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

#[derive(Resource)]
struct GradientPipeline {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

fn setup_renderer(
    mut commands: Commands,
    primary_window: Query<(Entity, &Window, &RawHandleWrapper), With<PrimaryWindow>>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    info!("Start renderer setup");
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
            required_limits: Limits::default().using_resolution(adapter.limits()),
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

    //let swapchain_capabilities = surface.get_capabilities(&adapter);
    //let swapchain_format = swapchain_capabilities.formats[0];

    // TODO prepare shader/pipeline in separate system
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
        label: Some("Triangle Pipeline"),
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
            // TODO store format in const
            targets: &[Some(MAIN_TEXTURE_FORMAT.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let gradient_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("compute_gradient.wgsl"))),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &BindGroupLayoutEntries::single(
            ShaderStages::COMPUTE,
            texture_storage_2d(MAIN_TEXTURE_FORMAT, wgpu::StorageTextureAccess::WriteOnly),
        ),
    });
    let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Gradient compute pipeline"),
        layout: Some(&compute_pipeline_layout),
        module: &gradient_shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    commands.insert_resource(GradientPipeline {
        pipeline: compute_pipeline,
        layout: bind_group_layout,
    });

    let blit_pipeline = init_blit_pipeline(&device, config.format);
    commands.insert_resource(blit_pipeline);

    commands.insert_resource(Device(device));
    commands.insert_resource(Queue(queue));
    commands.insert_resource(SurfaceConfiguration(config));
    commands.insert_resource(Surface(surface));
    commands.insert_resource(TrianglePipeline(render_pipeline));

    winit_window.set_visible(true);

    info!("Renderer setup done!");
}

fn init_blit_pipeline(device: &wgpu::Device, target_format: TextureFormat) -> BlitPipeline {
    let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("blit"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("blit.wgsl"))),
    });

    let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("blit"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &blit_shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &blit_shader,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(target_format.into())],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        // TODO MSAA
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("blit_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let blit_bind_group_layout = blit_pipeline.get_bind_group_layout(0);

    BlitPipeline {
        pipeline: blit_pipeline,
        sampler,
        layout: blit_bind_group_layout,
    }
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
    render_pipeline: Res<TrianglePipeline>,
    frame_count: Res<FrameCount>,
    mut main_texture_cache: Local<Option<(wgpu::Texture, wgpu::TextureView)>>,
    blit_pipeline: Res<BlitPipeline>,
    gradient_pipeline: Res<GradientPipeline>,
) {
    let frame = surface
        .get_current_texture()
        .expect("Failed to get texture");
    let view = frame.texture.create_view(&TextureViewDescriptor::default());
    if main_texture_cache.is_none() {
        let main_texture_format = MAIN_TEXTURE_FORMAT;
        let main_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Main Texture"),
            size: frame.texture.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: main_texture_format,
            usage: TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST
                | TextureUsages::STORAGE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING,
            view_formats: &[main_texture_format],
        });
        let main_texture_view = main_texture.create_view(&TextureViewDescriptor::default());
        *main_texture_cache = Some((main_texture, main_texture_view));
    }
    let Some((_main_texture, main_texture_view)) = main_texture_cache.as_ref() else {
        panic!("Failed to get main texture");
    };

    let mut command_encoder =
        device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    let flash = (frame_count.0 as f32 / 120.0).sin().abs();
    let clear_color = wgpu::Color {
        r: 0.0,
        g: 0.0,
        b: flash as f64,
        a: 1.0,
    };

    // Main render pass
    //{
    //    let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
    //        label: Some("Main Render Pass"),
    //        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
    //            view: &main_texture_view,
    //            resolve_target: None,
    //            ops: wgpu::Operations {
    //                load: wgpu::LoadOp::Clear(clear_color),
    //                store: wgpu::StoreOp::Store,
    //            },
    //        })],
    //        depth_stencil_attachment: None,
    //        occlusion_query_set: None,
    //        timestamp_writes: None,
    //    });
    //    rpass.set_pipeline(&render_pipeline);
    //    rpass.draw(0..3, 0..1);
    //}

    // Gradient compute
    {
        let gradient_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Gradient Compute Bind Group"),
            layout: &gradient_pipeline.layout,
            entries: &BindGroupEntries::sequential((BindingResource::TextureView(
                main_texture_view,
            ),)),
        });

        let mut compute_pass =
            command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());

        compute_pass.set_pipeline(&gradient_pipeline.pipeline);
        compute_pass.set_bind_group(0, &gradient_bind_group, &[]);
        // TODO extract extent
        compute_pass.dispatch_workgroups(
            (frame.texture.size().width as f32 / 16.0).ceil() as u32,
            (frame.texture.size().height as f32 / 16.0).ceil() as u32,
            1,
        );
    }

    // Blit main texture to swapchain
    {
        // TODO cache bind group
        let blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blit Bind Group"),
            layout: &blit_pipeline.layout,
            entries: &BindGroupEntries::sequential((
                BindingResource::TextureView(main_texture_view),
                BindingResource::Sampler(&blit_pipeline.sampler),
            )),
        });
        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blit Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&blit_pipeline.pipeline);
        rpass.set_bind_group(0, &blit_bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }

    queue.submit(Some(command_encoder.finish()));
    frame.present();
}
