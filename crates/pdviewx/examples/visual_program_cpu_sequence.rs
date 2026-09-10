//! Produces a small deterministic CPU-reference visual-program sequence.
//!
//! This is useful on hosts without a GPU adapter: it exercises the same typed
//! graph and evaluator used by the render differential tests and emits one
//! inspectable PNG per presentation-time sample.  A GPU-enabled host should
//! use `headless_smoke`/`occupancy_smoke` for adapter-backed images.

use pdviewx::{VisualInputs, VisualProgramBuilder, VisualStyle};
use std::error::Error;
use std::fs::File;
use std::path::{Path, PathBuf};

const WIDTH: u16 = 320;
const HEIGHT: u16 = 200;
const FRAME_COUNT: u16 = 8;

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args().nth(1).map_or_else(
        || PathBuf::from("/private/tmp/pdviewx-visual-sequence"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&output)?;
    let style = visual_style()?;
    for frame in 0..FRAME_COUNT {
        let time = f32::from(frame) / f32::from(FRAME_COUNT - 1);
        let mut pixels = vec![0_u8; usize::from(WIDTH) * usize::from(HEIGHT) * 4];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let entity_index = y * WIDTH + x;
                let evaluation = style.evaluate(VisualInputs {
                    time_seconds: time * std::f32::consts::TAU,
                    entity_index: u32::from(entity_index),
                    ..VisualInputs::default()
                });
                let offset = usize::from(y * WIDTH + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(&[
                    to_byte(evaluation.base_color[0] + evaluation.emission[0]),
                    to_byte(evaluation.base_color[1] + evaluation.emission[1]),
                    to_byte(evaluation.base_color[2] + evaluation.emission[2]),
                    to_byte(evaluation.opacity),
                ]);
            }
        }
        write_png(output.join(format!("visual-{frame:03}.png")), &pixels)?;
    }
    println!(
        "wrote {FRAME_COUNT} CPU-reference visual-program frames to {}",
        output.display()
    );
    Ok(())
}

fn visual_style() -> Result<VisualStyle, pdviewx::VisualError> {
    let mut builder = VisualProgramBuilder::new();
    let red = builder.color([0.95, 0.08, 0.06, 1.0])?;
    let blue = builder.color([0.05, 0.32, 0.98, 1.0])?;
    let index = builder.entity_index()?;
    let width = builder.scalar(f32::from(WIDTH * HEIGHT - 1))?;
    let position = builder.safe_divide(index, width)?;
    let color = builder.mix_color(red, blue, position)?;
    builder.set_base_color(color)?;

    let time = builder.time()?;
    let pulse = builder.sine(time)?;
    let one = builder.scalar(1.0)?;
    let half = builder.scalar(0.5)?;
    let shifted = builder.add(pulse, one)?;
    let emission_weight = builder.multiply(shifted, half)?;
    let black = builder.color([0.0, 0.0, 0.0, 1.0])?;
    let emission = builder.mix_color(black, color, emission_weight)?;
    builder.set_emission(emission)?;
    Ok(VisualStyle::new(builder.finish()?))
}

fn to_byte(value: f32) -> u8 {
    let rounded = (value.clamp(0.0, 1.0) * 255.0).round();
    match num_traits::cast(rounded) {
        Some(byte) => byte,
        None => 0,
    }
}

fn write_png(path: impl AsRef<Path>, pixels: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, u32::from(WIDTH), u32::from(HEIGHT));
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;
    Ok(())
}
