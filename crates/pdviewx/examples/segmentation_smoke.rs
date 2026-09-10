//! Deterministic categorical-volume visual and performance probe.
//!
//! Usage: `segmentation_smoke [output.png] [direct|hash] [full|slice] [frames]`

use pdviewx::{
    Camera, ClipPlane, Engine, EngineConfig, Image, ImageConfig, Representation, Rgba8, Scene,
    SegmentStyle, SegmentStyleTable, SegmentationStyle, SegmentedVolume, Vec3, VolumeSlice,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

const GRID: u16 = 64;
const IMAGE: ImageConfig = ImageConfig {
    width: 960,
    height: 720,
};
const DIRECT_LABELS: [u32; 4] = [1, 2, 3, 4];
const HASH_LABELS: [u32; 4] = [1_009, 65_537, 1_000_003, 4_000_009];

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let output = arguments.first().map_or(
        "target/visual-checks/segmentation-direct.png",
        String::as_str,
    );
    let lookup = arguments.get(1).map_or("direct", String::as_str);
    let slice = match arguments.get(2).map(String::as_str) {
        None | Some("full") => false,
        Some("slice") => true,
        Some(value) => return Err(format!("unknown segmentation view {value}").into()),
    };
    let frames = parse_frames(arguments.get(3))?;
    let labels = match lookup {
        "direct" => DIRECT_LABELS,
        "hash" => HASH_LABELS,
        value => return Err(format!("unknown segmentation lookup {value}").into()),
    };
    let volume = synthetic_segmentation(labels)?;
    let camera = Camera::framing_aabb(&volume.world_aabb(), 4.0 / 3.0);
    let mut scene = Scene::new();
    let volume = scene.add_segmented_volume(volume);
    let mut style = SegmentationStyle {
        styles: segment_styles(labels)?,
        opacity_scale: 0.92,
        step_scale: 0.50,
        ..SegmentationStyle::default()
    };
    if slice {
        style.slice = Some(VolumeSlice::new(ClipPlane::from_point_normal(
            Vec3::ZERO,
            Vec3::new(0.24, -0.12, 1.0),
        )?));
    }
    scene.represent(
        volume,
        Representation::segmentation().segmentation_style(style),
    )?;
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    write_png(output, &engine.render_image(&scene, &camera, IMAGE)?)?;
    if frames > 0 {
        profile(&mut engine, &scene, &camera, frames)?;
    }
    println!("segmentation lookup={lookup} slice={slice}; wrote {output}");
    Ok(())
}

fn synthetic_segmentation(labels: [u32; 4]) -> Result<SegmentedVolume, Box<dyn Error>> {
    let mut values = Vec::with_capacity(usize::from(GRID).pow(3));
    let center = (f32::from(GRID) - 1.0) * 0.5;
    for z in 0..GRID {
        for y in 0..GRID {
            for x in 0..GRID {
                let point = Vec3::new(
                    f32::from(x) - center,
                    f32::from(y) - center,
                    f32::from(z) - center,
                ) / center;
                let lobe = if (point - Vec3::new(-0.32, 0.18, 0.04)).length() < 0.48 {
                    labels[0]
                } else if (point - Vec3::new(0.32, 0.18, -0.04)).length() < 0.44 {
                    labels[1]
                } else if (point - Vec3::new(-0.08, -0.34, 0.20)).length() < 0.36 {
                    labels[2]
                } else if (point - Vec3::new(0.18, -0.24, -0.30)).length() < 0.32 {
                    labels[3]
                } else {
                    0
                };
                values.push(lobe);
            }
        }
    }
    let spacing = Vec3::splat(0.12);
    Ok(SegmentedVolume::from_spacing(
        [u32::from(GRID); 3],
        Vec3::splat(-center * spacing.x),
        spacing,
        Arc::from(values),
    )?)
}

fn segment_styles(labels: [u32; 4]) -> Result<SegmentStyleTable, pdviewx::CoreError> {
    SegmentStyleTable::new(&[
        SegmentStyle::new(labels[0], Rgba8::opaque(56, 189, 248), 0.76),
        SegmentStyle::new(labels[1], Rgba8::opaque(244, 114, 182), 0.72),
        SegmentStyle::new(labels[2], Rgba8::opaque(74, 222, 128), 0.82),
        SegmentStyle::new(labels[3], Rgba8::opaque(251, 191, 36), 0.78),
    ])
}

fn profile(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
    frames: usize,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..3 {
        engine.profile_frame(scene, camera, IMAGE)?;
    }
    let mut cpu = Vec::with_capacity(frames);
    let mut frame = Vec::with_capacity(frames);
    for _ in 0..frames {
        let timing = engine.profile_frame(scene, camera, IMAGE)?;
        cpu.push(timing.cpu_ns);
        frame.push(timing.frame_ns);
    }
    cpu.sort_unstable();
    frame.sort_unstable();
    let frame_p99 = percentile(&frame, 99);
    println!(
        "CPU median {} ns; frame median {} ns; p99 {} ns ({:.2} FPS)",
        percentile(&cpu, 50),
        percentile(&frame, 50),
        frame_p99,
        fps(frame_p99)
    );
    Ok(())
}

fn percentile(sorted: &[u64], value: usize) -> u64 {
    let row = (value * sorted.len()).div_ceil(100).saturating_sub(1);
    sorted.get(row).copied().map_or(0, |sample| sample)
}

fn fps(nanoseconds: u64) -> f64 {
    1.0 / std::time::Duration::from_nanos(nanoseconds.max(1)).as_secs_f64()
}

fn parse_frames(value: Option<&String>) -> Result<usize, io::Error> {
    value.map_or(Ok(0), |text| {
        text.parse::<usize>()
            .map_err(|error| io::Error::other(format!("invalid frame count: {error}")))
    })
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
