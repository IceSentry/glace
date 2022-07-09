use bevy::prelude::Color;
use image::{DynamicImage, Rgba, RgbaImage};

pub fn image_from_color(color: Color) -> RgbaImage {
    let mut rgba = RgbaImage::new(1, 1);
    rgba.put_pixel(
        0,
        0,
        Rgba([
            (color.r() * 255.0) as u8,
            (color.g() * 255.0) as u8,
            (color.b() * 255.0) as u8,
            (color.a() * 255.0) as u8,
        ]),
    );
    DynamicImage::ImageRgba8(rgba).to_rgba8()
}
