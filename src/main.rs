#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use std::borrow::Cow;

use bevy::{
    a11y::AccessibilityPlugin,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::system::NonSendMarker,
    input::InputPlugin,
    log::LogPlugin,
    prelude::*,
    render::render_resource::{
        BindGroupEntries, BindGroupLayoutEntries, binding_types::texture_storage_2d,
    },
    window::{PresentMode, PrimaryWindow, RawHandleWrapper, WindowResized, WindowResolution},
    winit::{WINIT_WINDOWS, WinitPlugin},
};
use egui_plugin::{EguiPlugin, EguiScreenDesciptorRes};
use gltf_loader::load_gltf;
use mesh::{GpuMeshBuffers, MeshPlugin, Vertex, upload_mesh};
use wgpu::{
    BindingResource, CompareFunction, DepthStencilState, ExperimentalFeatures, Features,
    InstanceDescriptor, LoadOp, MemoryHints, Operations, PushConstantRange,
    RenderPassDepthStencilAttachment, ShaderStages, StoreOp, TextureFormat, TextureUsages,
    TextureViewDescriptor,
    util::{TextureBlitter, TextureBlitterBuilder},
};

mod buffer_vec;
mod egui_plugin;
mod gltf_loader;
mod mesh;
mod render_context;
mod schedule;
mod ui;

use winit::dpi::PhysicalSize;

use crate::{
    egui_plugin::egui_render_pass,
    render_context::{PendingCommandBuffers, RenderContext},
    schedule::GlaceRender,
};

const MAIN_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            WindowPlugin {
                primary_window: Some(Window {
                    title: "glace2".into(),
                    present_mode: PresentMode::AutoNoVsync,
                    // Hide the window until the gpu is ready to draw
                    visible: false,
                    resolution: {
                        let mut res = WindowResolution::new(1280, 720);
                        // All this forced scale factor thing is because macos defaults to a really
                        // high scale factor
                        res.set_scale_factor_override(Some(1.0));
                        res.set_scale_factor(1.0);
                        res
                    },
                    ..default()
                }),
                ..default()
            },
            AccessibilityPlugin,
            WinitPlugin::default(),
            FrameTimeDiagnosticsPlugin::default(),
            InputPlugin,
            LogPlugin::default(),
            EguiPlugin,
            MeshPlugin,
        ))
        .add_schedule(schedule::GlaceRender::schedule())
        .add_systems(Startup, (setup_renderer, load_assets).chain())
        .add_systems(
            Update,
            (
                quit_on_q,
                update_window_title,
                // ui::ui,
            ),
        )
        .add_systems(Last, (resize, run_glace_render_schedule).chain())
        .add_systems(
            GlaceRender,
            (
                prepare_main_texture,
                ApplyDeferred,
                main_pass,
                blit,
                // egui_render_pass,
                ApplyDeferred,
                submit,
            )
                .chain(),
        )
        .insert_resource(ComputePushConstants {
            data1: Vec4::new(1.0, 1.0, 0.0, 1.0),
            data2: Vec4::new(0.0, 1.0, 0.0, 1.0),
        })
        .run();
}

fn quit_on_q(input: Res<ButtonInput<KeyCode>>, mut app_exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::KeyQ) {
        app_exit.write_default();
    }
}

#[derive(Resource, Deref, DerefMut)]
pub struct Device(pub wgpu::Device);

#[derive(Resource, Deref, DerefMut)]
struct Queue(wgpu::Queue);

#[derive(Resource, Deref, DerefMut)]
struct SurfaceConfiguration(wgpu::SurfaceConfiguration);

#[derive(Resource, Deref, DerefMut)]
struct Surface(wgpu::Surface<'static>);

#[derive(Resource, Deref, DerefMut)]
struct SwapchainTextureBlitter(TextureBlitter);

#[derive(Resource)]
struct GradientPipeline {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}
#[derive(Resource)]
struct MeshPipeline {
    pipeline: wgpu::RenderPipeline,
    //layout: wgpu::BindGroupLayout,
}

#[derive(Resource, bytemuck::NoUninit, Clone, Copy)]
#[repr(C)]
struct ComputePushConstants {
    data1: Vec4,
    data2: Vec4,
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

fn setup_renderer(
    mut commands: Commands,
    primary_window: Query<(Entity, &Window, &RawHandleWrapper), With<PrimaryWindow>>,
    _marker: NonSendMarker,
) {
    info!("Start renderer setup");

    let (window_entity, window, raw_handle_wrapper) = primary_window
        .single()
        .expect("Failed to get primary window during setup");
    WINIT_WINDOWS.with_borrow_mut(|winit_windows| {
        let winit_window = winit_windows
            .get_window(window_entity)
            .expect("Failed to get winit window");

        let mut size = winit_window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let instance = wgpu::Instance::new(&InstanceDescriptor::from_env_or_default());
        let surface = instance
            .create_surface(unsafe { raw_handle_wrapper.get_handle() })
            .expect("Failed to create surface");

        let adapter = futures_lite::future::block_on(instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ))
        .expect("Failed to request adapter");

        let (device, queue) =
            futures_lite::future::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: Features::PUSH_CONSTANTS,
                required_limits: adapter.limits(),
                label: Some("RenderDevice"),
                memory_hints: MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
                // SAFETY: YOLO
                experimental_features: unsafe { ExperimentalFeatures::enabled() },
            }))
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

        let mesh_pipeline = init_mesh_pipeline(&device);
        commands.insert_resource(mesh_pipeline);

        let gradient_pipeline = init_compute_gradient_pipeline(&device);
        commands.insert_resource(gradient_pipeline);

        let swapchain_format = surface.get_capabilities(&adapter).formats[0];
        commands.insert_resource(SwapchainTextureBlitter(
            TextureBlitterBuilder::new(&device, swapchain_format).build(),
        ));

        commands.insert_resource(Device(device));
        commands.insert_resource(Queue(queue));
        commands.insert_resource(SurfaceConfiguration(config));
        commands.insert_resource(Surface(surface));

        commands.insert_resource(PendingCommandBuffers::default());

        // At this point, the gpu is ready to draw so we can make the window visible
        winit_window.set_visible(true);

        info!("Renderer setup done!");
    });
}

#[derive(Component)]
struct GpuMesh(GpuMeshBuffers);

fn load_assets(mut commands: Commands, device: Res<Device>, queue: Res<Queue>) {
    let meshes = load_gltf("assets/models/gltf/basicmesh.glb", true);
    for mesh in meshes {
        if mesh.name == Some(String::from("Suzanne")) {
            println!("uploading mesh: {:?}", mesh.name);
            let mesh = crate::mesh::Mesh {
                vertices: mesh.vertices,
                indices: mesh.indices,
            };
            let gpu_buffers = upload_mesh(&device, &queue, &mesh);
            commands.spawn(GpuMesh(gpu_buffers));
        }
    }
}

fn init_mesh_pipeline(device: &wgpu::Device) -> MeshPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("mesh.wgsl"))),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[PushConstantRange {
            stages: ShaderStages::VERTEX_FRAGMENT,
            range: 0..std::mem::size_of::<GpuDrawPushConstants>() as u32,
        }],
    });
    // TODO consider making a builder thing
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Mesh Opaque Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex"),
            buffers: &[Vertex::layout()],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: MAIN_TEXTURE_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            ..default()
        },
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: CompareFunction::GreaterEqual,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    //let bind_group_layout = render_pipeline.get_bind_group_layout(0);
    MeshPipeline {
        pipeline: render_pipeline,
        //layout: bind_group_layout,
    }
}

fn init_compute_gradient_pipeline(device: &wgpu::Device) -> GradientPipeline {
    let gradient_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("compute_gradient.wgsl"))),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Compute Gradient Bind Group Layout"),
        entries: &BindGroupLayoutEntries::single(
            ShaderStages::COMPUTE,
            texture_storage_2d(MAIN_TEXTURE_FORMAT, wgpu::StorageTextureAccess::WriteOnly),
        ),
    });
    let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[PushConstantRange {
            stages: ShaderStages::COMPUTE,
            range: 0..std::mem::size_of::<ComputePushConstants>() as u32,
        }],
    });
    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Gradient Pipeline"),
        layout: Some(&compute_pipeline_layout),
        module: &gradient_shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    GradientPipeline {
        pipeline: compute_pipeline,
        layout: bind_group_layout,
    }
}

fn resize(
    mut surface_config: ResMut<SurfaceConfiguration>,
    surface: Res<Surface>,
    device: Res<Device>,
    mut window_resized: MessageReader<WindowResized>,
    windows: Query<&Window>,
    mut egui_screen_descriptor: ResMut<EguiScreenDesciptorRes>,
) {
    for event in window_resized.read() {
        let window = windows.get(event.window).expect("window not found");
        let width = window.physical_width();
        let height = window.physical_height();

        let new_size = PhysicalSize { width, height };

        if new_size.width > 0 && new_size.height > 0 {
            surface_config.width = new_size.width;
            surface_config.height = new_size.height;

            surface.configure(&device, &surface_config);

            egui_screen_descriptor.0.size_in_pixels = [new_size.width, new_size.height];
        }
    }
}

#[derive(Resource, bytemuck::NoUninit, Clone, Copy)]
#[repr(C)]
struct GpuDrawPushConstants {
    world_matrix: Mat4,
}

fn run_glace_render_schedule(world: &mut World) {
    world.run_schedule(GlaceRender);
}

#[derive(Resource)]
struct MainTexture(wgpu::Texture, wgpu::TextureView);

#[derive(Resource)]
struct DepthTexture(wgpu::Texture, wgpu::TextureView);

#[derive(Resource)]
struct SurfaceTexture(Option<wgpu::SurfaceTexture>, wgpu::TextureView);

fn prepare_main_texture(
    mut commands: Commands,
    surface: Res<Surface>,
    device: Res<Device>,
    mut main_texture: Option<ResMut<MainTexture>>,
    mut depth_texture: Option<ResMut<DepthTexture>>,
    mut window_resized_messsages: MessageReader<WindowResized>,
) {
    let surface_texture = surface
        .get_current_texture()
        .expect("Failed to get current texture");

    let main_texture_format = MAIN_TEXTURE_FORMAT;
    let desc = wgpu::TextureDescriptor {
        label: Some("Main Texture"),
        size: surface_texture.texture.size(),
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
    };
    let depth_texture_desc = wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: surface_texture.texture.size(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TextureFormat::Depth32Float,
        usage: TextureUsages::COPY_SRC
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };

    if let Some(main_texture) = main_texture.as_mut()
        && let Some(depth_texture) = depth_texture.as_mut()
    {
        if window_resized_messsages.read().count() > 0 {
            let new_main_texture = device.create_texture(&desc);
            let new_main_texture_view =
                new_main_texture.create_view(&TextureViewDescriptor::default());
            main_texture.0 = new_main_texture;
            main_texture.1 = new_main_texture_view;

            let new_depth_texture = device.create_texture(&depth_texture_desc);
            let new_depth_texture_view =
                new_depth_texture.create_view(&TextureViewDescriptor::default());
            depth_texture.0 = new_depth_texture;
            depth_texture.1 = new_depth_texture_view;
        }
    } else {
        let main_texture = device.create_texture(&desc);
        let main_texture_view = main_texture.create_view(&TextureViewDescriptor::default());
        commands.insert_resource(MainTexture(main_texture, main_texture_view));

        let depth_texture = device.create_texture(&depth_texture_desc);
        let depth_texture_view = depth_texture.create_view(&TextureViewDescriptor::default());
        commands.insert_resource(DepthTexture(depth_texture, depth_texture_view));
    }

    // TODO just update the existing resource
    let view = surface_texture
        .texture
        .create_view(&TextureViewDescriptor::default());
    commands.insert_resource(SurfaceTexture(Some(surface_texture), view));
}

fn main_pass(
    mut ctx: RenderContext,
    device: Res<Device>,
    gradient_pipeline: Res<GradientPipeline>,
    compute_push_constants: Res<ComputePushConstants>,
    main_texture: Res<MainTexture>,
    depth_texture: Res<DepthTexture>,
    (mesh_pipeline, meshes): (Res<MeshPipeline>, Query<&GpuMesh>),
    mut camera_transform: Local<Option<Transform>>,
) {
    let main_texture_view = main_texture.1.clone();

    let draw_extent = main_texture.0.size();

    let command_encoder = ctx.command_encoder();
    command_encoder.push_debug_group("main_pass");

    // Gradient compute
    {
        #[cfg(feature = "trace")]
        let _span = info_span!("compute gradient").entered();
        command_encoder.push_debug_group("compute gradient");

        let gradient_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Gradient Compute Bind Group"),
            layout: &gradient_pipeline.layout,
            entries: &BindGroupEntries::sequential((BindingResource::TextureView(
                &main_texture_view,
            ),)),
        });

        {
            let mut compute_pass =
                command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());

            compute_pass.set_pipeline(&gradient_pipeline.pipeline);
            compute_pass.set_bind_group(0, &gradient_bind_group, &[]);
            compute_pass.set_push_constants(0, bytemuck::bytes_of(&*compute_push_constants));
            compute_pass.dispatch_workgroups(
                (main_texture.0.width() as f32 / 16.0).ceil() as u32,
                (main_texture.0.height() as f32 / 16.0).ceil() as u32,
                1,
            );
        }

        command_encoder.pop_debug_group();
    }

    // Main opaque pass
    {
        #[cfg(feature = "trace")]
        let _span = info_span!("main opaque mesh").entered();

        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Opaque Mesh Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &main_texture_view.clone(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &depth_texture.1,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(0.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        rpass.set_viewport(
            0.0,
            0.0,
            draw_extent.width as f32,
            draw_extent.height as f32,
            0.0,
            1.0,
        );
        rpass.set_scissor_rect(0, 0, draw_extent.width, draw_extent.height);

        rpass.set_pipeline(&mesh_pipeline.pipeline);

        // TODO create a camera entity
        // let view = Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0));
        if camera_transform.is_none() {
            let transform = Transform::from_translation(Vec3::new(0.0, 0.0, -5.0));
            *camera_transform = Some(transform);
        }
        let view = camera_transform.unwrap().compute_affine();

        let projection = Mat4::perspective_infinite_reverse_rh(
            70.0,
            draw_extent.width as f32 / draw_extent.height as f32,
            0.1,
        );
        let world_matrix = projection * view;
        let push_constant = GpuDrawPushConstants { world_matrix };
        rpass.set_push_constants(
            ShaderStages::VERTEX_FRAGMENT,
            0,
            bytemuck::bytes_of(&push_constant),
        );

        for mesh in meshes {
            rpass.set_vertex_buffer(0, mesh.0.vertex_buffer.buffer().unwrap().slice(..));
            rpass.set_index_buffer(
                mesh.0.index_buffer.buffer().unwrap().slice(..),
                wgpu::IndexFormat::Uint32,
            );
            rpass.draw_indexed(0..mesh.0.index_buffer.len() as u32, 0, 0..1);
        }
    }

    command_encoder.pop_debug_group();
}

fn blit(
    mut ctx: RenderContext,
    device: Res<Device>,
    swapchain_blitter: Res<SwapchainTextureBlitter>,
    main_texture_cache: Res<MainTexture>,
    surface_texture: Res<SurfaceTexture>,
) {
    swapchain_blitter.copy(
        &device,
        ctx.command_encoder(),
        &main_texture_cache.1,
        &surface_texture.1,
    );
}

fn submit(
    queue: Res<Queue>,
    mut pending: ResMut<PendingCommandBuffers>,
    mut surface_texture: ResMut<SurfaceTexture>,
) {
    let buffers = pending.take();
    for b in buffers {
        queue.submit(Some(b));
    }
    surface_texture.0.take().unwrap().present();
}
