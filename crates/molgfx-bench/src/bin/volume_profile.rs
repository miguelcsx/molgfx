//! Whole-graph volume benchmark using a deterministic scalar grid.
//!
//! Usage: `volume_profile [direct|isosurface|medium|slice|liquid] [output.png]`

use molgfx::{
    core::{
        ClipPlane, Representation, ScalarVolume, Scene, VolumeSlice, VolumeStyle,
        VolumeTransferFunction, VolumeTransferPoint,
    },
    math::{Camera, Rgba8, Vec3},
    render::{Engine, EngineConfig, Image, ImageConfig},
};
use molgfx_bench::{CumulativeTelemetry, FrameSample, summarize};
use std::error::Error;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

const GRID: u16 = 96;
const WARMUP_FRAMES: usize = 20;
const MEASURED_FRAMES: usize = 120;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let algorithm = arguments.first().map_or("direct", String::as_str);
    let mut scene = Scene::new();
    let volume = scene.add_volume(synthetic_density()?);
    let volume_style = match algorithm {
        "direct" => VolumeStyle::default(),
        "isosurface" => VolumeStyle::isosurface(),
        "medium" => VolumeStyle::medium(),
        "slice" => VolumeStyle::slice(VolumeSlice::new(ClipPlane::from_point_normal(
            Vec3::ZERO,
            Vec3::Z,
        )?)),
        "liquid" => VolumeStyle::liquid_surface(),
        name => return Err(format!("unknown volume algorithm {name}").into()),
    };
    let representation =
        scene.represent(volume, Representation::volume().volume_style(volume_style))?;
    let Some(style) = scene.representation_mut(representation) else {
        return Err("new volume representation became stale".into());
    };
    style.volume.opacity_scale = if algorithm == "medium" { 0.60 } else { 1.35 };
    style.volume.step_scale = if algorithm == "medium" { 0.65 } else { 0.45 };
    style.volume.transfer = transfer()?;
    if algorithm == "isosurface" {
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
    let counters = engine.residency_counters();
    let mut previous = CumulativeTelemetry {
        allocation_events: counters.allocation_events,
        upload_bytes: counters.upload_bytes,
        resident_bytes: counters.resident_bytes,
        stall_events: counters.stall_events,
    };
    let mut samples = Vec::with_capacity(MEASURED_FRAMES);
    for _ in 0..MEASURED_FRAMES {
        let timing = engine.profile_frame(&scene, &camera, config)?;
        let counters = timing.residency_counters();
        let current = CumulativeTelemetry {
            allocation_events: counters.allocation_events,
            upload_bytes: counters.upload_bytes,
            resident_bytes: counters.resident_bytes,
            stall_events: counters.stall_events,
        };
        samples.push(FrameSample::measured(
            timing.gpu_ns,
            timing.cpu_ns,
            timing.frame_ns,
            previous,
            current,
        )?);
        previous = current;
    }
    let summary = summarize(&samples, &mut Vec::with_capacity(samples.len()))?;
    println!("grid={GRID}x{GRID}x{GRID}");
    println!("algorithm={algorithm}");
    println!("frames={MEASURED_FRAMES}");
    println!("gpu_median_ns={}", summary.gpu_median_ns);
    println!("gpu_p99_ns={}", summary.gpu_p99_ns);
    println!("cpu_median_ns={}", summary.cpu_median_ns);
    println!("frame_median_ns={}", summary.frame_median_ns);
    println!("frame_p99_ns={}", summary.frame_p99_ns);
    println!("frame_p99_fps={:.2}", fps(summary.frame_p99_ns));
    println!("gpu_median_fps={:.2}", fps(summary.gpu_median_ns));
    println!("gpu_p99_fps={:.2}", fps(summary.gpu_p99_ns));
    println!("max_allocation_events={}", summary.max_allocations);
    println!("max_upload_bytes={}", summary.max_upload_bytes);
    println!("peak_resident_bytes={}", summary.peak_resident_bytes);
    println!("max_stall_events={}", summary.max_stall_events);
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

fn transfer() -> Result<VolumeTransferFunction, molgfx::core::CoreError> {
    VolumeTransferFunction::new(&[
        VolumeTransferPoint::new(0.10, Rgba8::opaque(30, 118, 180), 0.0),
        VolumeTransferPoint::new(0.26, Rgba8::opaque(52, 191, 206), 0.025),
        VolumeTransferPoint::new(0.62, Rgba8::opaque(160, 218, 166), 0.22),
        VolumeTransferPoint::new(1.15, Rgba8::opaque(255, 213, 79), 0.82),
    ])
}

fn synthetic_density() -> Result<ScalarVolume, molgfx::core::CoreError> {
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
    ScalarVolume::from_spacing(
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
    1.0 / std::time::Duration::from_nanos(nanoseconds).as_secs_f64()
}
