//! Candidate image output and camera geometry boundaries.

use molgfx::render::Image;
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

pub(crate) fn write_png(path: &Path, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

pub(crate) fn dimension_aspect(width: u32, height: u32) -> Result<f32, io::Error> {
    let width = num_traits::cast::<u32, f32>(width)
        .ok_or_else(|| io::Error::other("width is not representable as f32"))?;
    let height = num_traits::cast::<u32, f32>(height)
        .ok_or_else(|| io::Error::other("height is not representable as f32"))?;
    Ok(width / height)
}
