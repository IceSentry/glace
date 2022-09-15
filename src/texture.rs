use image::DynamicImage;

#[derive(Debug)]
pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    #[allow(unused)]
    pub fn default_white(device: &wgpu::Device, queue: &wgpu::Queue) -> anyhow::Result<Self> {
        Self::solid_color(device, queue, [255, 255, 255])
    }

    /// Color components must be in range 0-255
    #[allow(unused)]
    pub fn solid_color(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color: [u8; 3],
    ) -> anyhow::Result<Self> {
        use image::{Rgba, RgbaImage};

        let mut rgba = RgbaImage::new(1, 1);
        rgba.put_pixel(0, 0, Rgba([color[0], color[1], color[2], 255]));
        let rgba = DynamicImage::ImageRgba8(rgba).to_rgba8();

        Self::from_image(
            device,
            queue,
            &DynamicImage::ImageRgba8(rgba).to_rgba8(),
            Some("default_white"),
            None,
        )
    }

    #[allow(unused)]
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
        format: Option<wgpu::TextureFormat>,
    ) -> anyhow::Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img.to_rgba8(), Some(label), format)
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rgba: &image::RgbaImage,
        label: Option<&str>,
        format: Option<wgpu::TextureFormat>,
    ) -> anyhow::Result<Self> {
        let format = format.unwrap_or(wgpu::TextureFormat::Rgba8UnormSrgb);
        let (texture_width, texture_height) = rgba.dimensions();

        let size = wgpu::Extent3d {
            width: texture_width,
            height: texture_height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * texture_width),
                rows_per_image: std::num::NonZeroU32::new(texture_height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
        })
    }

    pub fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        sample_count: u32,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size,
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            compare: None,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }
}
