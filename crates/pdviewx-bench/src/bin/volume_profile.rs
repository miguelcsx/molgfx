//! Whole-graph direct-volume benchmark using a deterministic scalar grid.

use pdviewx::{
    Camera, DensityVolume, Engine, EngineConfig, Image, ImageConfig, Rgba8, Scene, Vec3,
    VolumeTransferFunction, VolumeTransferPoint,
};
use pdviewx_bench::{FrameSample, summarize};
use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

const GRID: u16 = 96;
const WARMUP_FRAMES: usize = 20;
const MEASURED_FRAMES: usize = 120;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let isosurface = arguments.first().is_some_and(|value| value == "isosurface");
    let mut scene = Scene::new();
    let volume = scene.add_volume(synthetic_density()?);
    let representation = if isosurface {
        scene.represent_isosurface(volume)?
    } else {
        scene.represent_volume(volume)?
    };
    let Some(style) = scene.representation_mut(representation) else {
        return Err("new volume representation became stale".into());
    };
    style.volume.opacity_scale = 1.35;
    style.volume.step_scale = 0.45;
    style.volume.transfer = transfer()?;
    if isosurface {
        style.params.isolevel = 0.38;
    }
    let config = ImageConfig {
        width: 1024,
        height: 768,
    };
    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    for _ in 0..WARMUP_FRAMES {
        engine.profile_frame(&scene, &camera, config)?;
    }
    let mut samples = Vec::with_capacity(MEASURED_FRAMES);
    for _ in 0..MEASURED_FRAMES {
        let timing = engine.profile_frame(&scene, &camera, config)?;
        samples.push(FrameSample {
            gpu_ns: timing.gpu_ns,
            cpu_ns: timing.cpu_ns,
            ..FrameSample::default()
        });
    }
    let summary = summarize(&samples, &mut Vec::with_capacity(samples.len()))?;
    println!("grid={GRID}x{GRID}x{GRID}");
    println!("frames={MEASURED_FRAMES}");
    println!("gpu_median_ns={}", summary.gpu_median_ns);
    println!("gpu_p99_ns={}", summary.gpu_p99_ns);
    println!("cpu_median_ns={}", summary.cpu_median_ns);
    println!("gpu_median_fps={:.2}", fps(summary.gpu_median_ns));
    println!("gpu_p99_fps={:.2}", fps(summary.gpu_p99_ns));
    if let Some(path) = arguments.get(1) {
        let image = engine.render_image(&scene, &camera, config)?;
        write_png(path, &image)?;
        println!("visual={path}");
    }
    Ok(())
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

fn transfer() -> Result<VolumeTransferFunction, pdviewx::CoreError> {
    VolumeTransferFunction::new(&[
        VolumeTransferPoint::new(0.10, Rgba8::opaque(30, 118, 180), 0.0),
        VolumeTransferPoint::new(0.26, Rgba8::opaque(52, 191, 206), 0.025),
        VolumeTransferPoint::new(0.62, Rgba8::opaque(160, 218, 166), 0.22),
        VolumeTransferPoint::new(1.15, Rgba8::opaque(255, 213, 79), 0.82),
    ])
}

fn synthetic_density() -> Result<DensityVolume, pdviewx::CoreError> {
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
                let field = gaussian(
                    point,
                    Vec3::new(-0.30, 0.02, 0.08),
                    Vec3::new(0.46, 0.31, 0.38),
                ) + gaussian(
                    point,
                    Vec3::new(0.34, 0.10, -0.08),
                    Vec3::new(0.36, 0.43, 0.30),
                ) * 0.88
                    + gaussian(point, Vec3::new(0.02, -0.40, 0.18), Vec3::splat(0.22)) * 0.52
                    + gaussian(
                        point,
                        Vec3::new(-0.08, 0.22, -0.29),
                        Vec3::new(0.13, 0.10, 0.16),
                    ) * 0.24;
                let pocket = gaussian(
                    point,
                    Vec3::new(0.02, 0.02, 0.20),
                    Vec3::new(0.17, 0.22, 0.18),
                );
                values.push((field - pocket * 0.46).max(0.0));
            }
        }
    }
    let spacing = Vec3::splat(0.22);
    DensityVolume::from_spacing(
        [u32::from(GRID); 3],
        Vec3::splat(-center * spacing.x),
        spacing,
        Arc::from(values),
    )
}

fn gaussian(point: Vec3, center: Vec3, sigma: Vec3) -> f32 {
    (-0.5 * ((point - center) / sigma).length_squared()).exp()
}

fn fps(nanoseconds: u64) -> f64 {
    if nanoseconds == 0 {
        return f64::INFINITY;
    }
    let bounded = u32::try_from(nanoseconds).map_or(u32::MAX, |value| value);
    1_000_000_000.0 / f64::from(bounded)
}
