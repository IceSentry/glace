#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use std::borrow::Cow;

use bevy::{
    a11y::AccessibilityPlugin,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::InputPlugin,
    log::LogPlugin,
    prelude::*,
    render::render_resource::{
        binding_types::texture_storage_2d, BindGroupEntries, BindGroupLayoutEntries, ShaderType,
    },
    window::{PresentMode, PrimaryWindow, RawHandleWrapper, WindowResized, WindowResolution},
    winit::{WakeUp, WinitPlugin, WinitWindows},
};
use egui_plugin::{
    egui_render_pass, EguiCtxRes, EguiPaintJobs, EguiPlugin, EguiRenderer, EguiScreenDesciptorRes,
    EguiWinitState,
};
use wgpu::{
    BindingResource, BufferUsages, CommandEncoderDescriptor, Features, MemoryHints,
    PushConstantRange, ShaderStages, StoreOp, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use winit::dpi::PhysicalSize;

mod buffer_vec;
mod egui_plugin;
mod ui;

use buffer_vec::BufferVec;

const MAIN_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            WindowPlugin {
                primary_window: Some(Window {
                    title: "glace2".into(),
                    present_mode: PresentMode::AutoVsync,
                    // Hide the window until the gpu is ready to draw
                    visible: false,
                    resolution: {
                        // All this forced scale factor thing is because macos defaults to a really
                        // high scale factor
                        let mut res = WindowResolution::new(1920.0, 1080.0);
                        res.set_scale_factor_override(Some(1.0));
                        res.set_scale_factor(1.0);
                        res
                    },
                    ..default()
                }),
                ..default()
            },
            AccessibilityPlugin,
            WinitPlugin::<WakeUp>::default(),
            FrameTimeDiagnosticsPlugin::default(),
            InputPlugin,
            LogPlugin::default(),
            EguiPlugin,
        ))
        .add_systems(Startup, setup_renderer)
        .add_systems(Update, (quit_on_q, update_window_title, ui::ui))
        .add_systems(PostUpdate, (resize, render).chain())
        .insert_resource(ComputePushConstants {
            data1: Vec4::new(1.0, 1.0, 0.0, 1.0),
            data2: Vec4::new(0.0, 1.0, 0.0, 1.0),
        })
        .run();
}

fn quit_on_q(input: Res<ButtonInput<KeyCode>>, mut exit_event: EventWriter<AppExit>) {
    if input.just_pressed(KeyCode::KeyQ) {
        exit_event.write_default();
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
pub struct Device(pub wgpu::Device);

#[derive(Resource, Deref, DerefMut)]
struct Queue(wgpu::Queue);

#[derive(Resource, Deref, DerefMut)]
struct SurfaceConfiguration(wgpu::SurfaceConfiguration);

#[derive(Resource, Deref, DerefMut)]
struct Surface(wgpu::Surface<'static>);

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

#[derive(Resource)]
struct RectangleBuffers(GpuMeshBuffers);

fn setup_renderer(
    mut commands: Commands,
    primary_window: Query<(Entity, &Window, &RawHandleWrapper), With<PrimaryWindow>>,
    winit_windows: NonSendMut<WinitWindows>,
) {
    info!("Start renderer setup");

    let (window_entity, window, raw_handle_wrapper) = primary_window
        .single()
        .expect("Failed to get primary window during setup");
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
            required_features: Features::PUSH_CONSTANTS,
            required_limits: adapter.limits(),
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

    let mesh_pipeline = init_mesh_pipeline_gradient_pipeline(&device);
    commands.insert_resource(mesh_pipeline);

    let gradient_pipeline = init_compute_gradient_pipeline(&device);
    commands.insert_resource(gradient_pipeline);

    let swapchain_format = surface.get_capabilities(&adapter).formats[0];
    let blit_pipeline = init_blit_pipeline(&device, swapchain_format);
    commands.insert_resource(blit_pipeline);

    let rectangle_buffers = init_default_data(&device, &queue);
    commands.insert_resource(RectangleBuffers(rectangle_buffers));

    commands.insert_resource(Device(device));
    commands.insert_resource(Queue(queue));
    commands.insert_resource(SurfaceConfiguration(config));
    commands.insert_resource(Surface(surface));

    // At this point, the gpu is ready to draw so we can make the window visible
    winit_window.set_visible(true);

    info!("Renderer setup done!");
}

fn init_mesh_pipeline_gradient_pipeline(device: &wgpu::Device) -> MeshPipeline {
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
            front_face: wgpu::FrontFace::Cw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            ..default()
        },
        depth_stencil: None,
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
    mut egui_screen_descriptor: ResMut<EguiScreenDesciptorRes>,
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

            egui_screen_descriptor.0.size_in_pixels = [new_size.width, new_size.height];
        }
    }
}

#[derive(Default, Clone, Copy, ShaderType)]
struct Vertex {
    position: Vec3,
    uv_x: f32,
    normal: Vec3,
    uv_y: f32,
    color: Vec4,
}

impl Vertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTESS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32,
            2 => Float32x3,
            3 => Float32,
            4 => Float32x4,
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTESS,
        }
    }
}

struct GpuMeshBuffers {
    index_buffer: BufferVec<u32>,
    vertex_buffer: BufferVec<Vertex>,
}

#[derive(Resource, bytemuck::NoUninit, Clone, Copy)]
#[repr(C)]
struct GpuDrawPushConstants {
    world_matrix: Mat4,
}

fn upload_mesh(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    indices: &[u32],
    vertices: &[Vertex],
) -> GpuMeshBuffers {
    let mut vertex_buffer = BufferVec::new(BufferUsages::STORAGE | BufferUsages::VERTEX);
    vertex_buffer.reserve(vertices.len(), device);
    for vertex in vertices.iter().copied() {
        vertex_buffer.push(vertex);
    }
    vertex_buffer.write_buffer(device, queue);

    let mut index_buffer = BufferVec::new(BufferUsages::INDEX);
    index_buffer.reserve(indices.len(), device);
    for index in indices.iter().copied() {
        index_buffer.push(index);
    }
    index_buffer.write_buffer(device, queue);

    GpuMeshBuffers {
        vertex_buffer,
        index_buffer,
    }
}

fn init_default_data(device: &wgpu::Device, queue: &wgpu::Queue) -> GpuMeshBuffers {
    let mut rect_vertices: [Vertex; 4] = Default::default();
    rect_vertices[0].position = Vec3::new(0.5, -0.5, 0.0);
    rect_vertices[1].position = Vec3::new(0.5, 0.5, 0.0);
    rect_vertices[2].position = Vec3::new(-0.5, -0.5, 0.0);
    rect_vertices[3].position = Vec3::new(-0.5, 0.5, 0.0);

    rect_vertices[0].color = Vec4::new(0.0, 0.0, 0.0, 1.0);
    rect_vertices[1].color = Vec4::new(0.5, 0.5, 0.5, 1.0);
    rect_vertices[2].color = Vec4::new(1.0, 0.0, 0.0, 1.0);
    rect_vertices[3].color = Vec4::new(0.0, 1.0, 0.0, 1.0);

    let mut rect_indices: [u32; 6] = Default::default();
    rect_indices[0] = 0;
    rect_indices[1] = 1;
    rect_indices[2] = 2;

    rect_indices[3] = 2;
    rect_indices[4] = 1;
    rect_indices[5] = 3;

    upload_mesh(device, queue, &rect_indices, &rect_vertices)
}

fn render(
    (surface, device, queue): (Res<Surface>, Res<Device>, Res<Queue>),
    mut main_texture_cache: Local<Option<(wgpu::Texture, wgpu::TextureView)>>,
    blit_pipeline: Res<BlitPipeline>,
    gradient_pipeline: Res<GradientPipeline>,
    windows: Query<Entity, With<Window>>,
    winit_windows: NonSend<WinitWindows>,
    screen_descriptor: Res<EguiScreenDesciptorRes>,
    mut egui_renderer: NonSendMut<EguiRenderer>,
    mut paint_jobs: ResMut<EguiPaintJobs>,
    egui_ctx: Res<EguiCtxRes>,
    mut state: ResMut<EguiWinitState>,
    mut window_resized_events: EventReader<WindowResized>,
    compute_push_constants: Res<ComputePushConstants>,
    (mesh_pipeline, rectangle_buffers): (Res<MeshPipeline>, Res<RectangleBuffers>),
) {
    let window = if let Ok(window) = windows.single() {
        winit_windows
            .get_window(window)
            .expect("Failed to get primary window")
    } else {
        return;
    };
    let frame = surface
        .get_current_texture()
        .expect("Failed to get texture");
    let view = frame.texture.create_view(&TextureViewDescriptor::default());
    // TODO better handle resize, maybe just use bevy's TextureCache
    if main_texture_cache.is_none() || window_resized_events.read().count() > 0 {
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

    let mut command_encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());

    let draw_extent = frame.texture.size();

    // Gradient compute
    {
        #[cfg(feature = "trace")]
        let _span = info_span!("compute gradient").entered();

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
        compute_pass.set_push_constants(0, bytemuck::bytes_of(&*compute_push_constants));
        compute_pass.dispatch_workgroups(
            (draw_extent.width as f32 / 16.0).ceil() as u32,
            (draw_extent.height as f32 / 16.0).ceil() as u32,
            1,
        );
    }

    // Main opaque mesh
    {
        #[cfg(feature = "trace")]
        let _span = info_span!("main opaque mesh").entered();

        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Opaque Mesh Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: main_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        let draw_extent = frame.texture.size();
        rpass.set_viewport(
            0.0,
            0.0,
            draw_extent.width as f32,
            draw_extent.height as f32,
            0.0,
            1.0,
        );
        rpass.set_scissor_rect(0, 0, draw_extent.width, draw_extent.height);

        let push_constant = GpuDrawPushConstants {
            world_matrix: Mat4::IDENTITY,
        };

        rpass.set_pipeline(&mesh_pipeline.pipeline);
        rpass.set_push_constants(
            ShaderStages::VERTEX_FRAGMENT,
            0,
            bytemuck::bytes_of(&push_constant),
        );
        rpass.set_vertex_buffer(
            0,
            rectangle_buffers
                .0
                .vertex_buffer
                .buffer()
                .unwrap()
                .slice(..),
        );
        rpass.set_index_buffer(
            rectangle_buffers.0.index_buffer.buffer().unwrap().slice(..),
            wgpu::IndexFormat::Uint32,
        );
        rpass.draw_indexed(0..rectangle_buffers.0.index_buffer.len() as u32, 0, 0..1);
        rpass.draw(0..3, 0..1);
    }

    // Blit main texture to swapchain
    {
        #[cfg(feature = "trace")]
        let _span = info_span!("blit").entered();

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

    // Render egui
    egui_render_pass(
        window,
        &mut egui_renderer,
        &mut paint_jobs,
        &egui_ctx,
        &mut state,
        &screen_descriptor,
        &device,
        &queue,
        &mut command_encoder,
        &view,
    );

    queue.submit(Some(command_encoder.finish()));
    frame.present();
}
