//! Whole-graph headless GPU benchmark using the production facade.

use molgfx::{
    AtomSelection, Camera, Engine, EngineConfig, Image, ImageConfig, RenderMode, RenderProfile,
    RepresentationKind, Scene, SurfaceKind, SurfaceStyle,
};
use molgfx_bench::{summarize, CumulativeTelemetry, FrameSample, FrameSummary};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

const WARMUP_FRAMES: usize = 20;
const DEFAULT_MEASURED_FRAMES: usize = 120;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(path) = arguments.first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: frame_profile STRUCTURE [representation] [frames] [opacity] [realtime|cinematic] [width] [height] [inspection|illustrative|cinematic] [output.png]",
        )
        .into());
    };
    let requested = arguments.get(1).map(String::as_str);
    let mut scene = load_scene(path, requested)?;
    configure_representation(&mut scene, requested, arguments.get(3))?;
    let config = ImageConfig {
        width: parse_dimension(arguments.get(5), 1024)?,
        height: parse_dimension(arguments.get(6), 768)?,
    };
    let camera = Camera::framing_aabb(&scene.world_aabb(), aspect_ratio(config));
    let measured_frames = parse_frames(arguments.get(2))?;
    let mode = match arguments.get(4).map(String::as_str) {
        Some("cinematic") => RenderMode::Cinematic,
        _ => RenderMode::Realtime,
    };
    let profile_name = arguments.get(7).map(String::as_str);
    let profile = render_profile(profile_name)?;
    let mut engine = Engine::new(
        &EngineConfig {
            mode,
            profile,
            ..EngineConfig::default()
        },
        None,
    )?;
    let atom_count = scene
        .structures()
        .map(|(_, placed)| u64::from(placed.atoms.len()))
        .sum::<u64>();
    for _ in 0..WARMUP_FRAMES {
        engine.profile_frame(&scene, &camera, config)?;
    }
    let mut previous = telemetry(&engine);
    let mut samples = Vec::with_capacity(measured_frames);
    for _ in 0..measured_frames {
        let timing = engine.profile_frame(&scene, &camera, config)?;
        let current = timing_telemetry(&timing);
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
    let profile_label = match profile_name {
        Some(name) => name,
        None => "inspection",
    };
    print_frame_summary(atom_count, measured_frames, profile_label, summary);
    let batch_summary = measure_batches(&mut engine, &scene, &camera, config, measured_frames)?;
    print_batch_summary(batch_summary);
    if let Some(path) = arguments.get(8) {
        let image = engine.render_image(&scene, &camera, config)?;
        write_png(path, &image)?;
        println!("visual={path}");
    }
    Ok(())
}

fn print_frame_summary(atoms: u64, frames: usize, profile: &str, summary: FrameSummary) {
    println!("atoms={atoms}");
    println!("frames={frames}");
    println!("profile={profile}");
    println!("gpu_median_ns={}", summary.gpu_median_ns);
    println!("gpu_p95_ns={}", summary.gpu_p95_ns);
    println!("gpu_p99_ns={}", summary.gpu_p99_ns);
    println!("cpu_median_ns={}", summary.cpu_median_ns);
    println!("cpu_p95_ns={}", summary.cpu_p95_ns);
    println!("cpu_p99_ns={}", summary.cpu_p99_ns);
    println!("frame_median_ns={}", summary.frame_median_ns);
    println!("frame_p95_ns={}", summary.frame_p95_ns);
    println!("frame_p99_ns={}", summary.frame_p99_ns);
    println!("frame_p99_fps={:.2}", fps(summary.frame_p99_ns));
    println!("gpu_median_fps={:.2}", fps(summary.gpu_median_ns));
    println!("gpu_p95_fps={:.2}", fps(summary.gpu_p95_ns));
    println!("gpu_p99_fps={:.2}", fps(summary.gpu_p99_ns));
    println!("max_allocation_events={}", summary.max_allocations);
    println!("max_upload_bytes={}", summary.max_upload_bytes);
    println!("peak_resident_bytes={}", summary.peak_resident_bytes);
    println!("max_stall_events={}", summary.max_stall_events);
}

fn measure_batches(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
    config: ImageConfig,
    measured_frames: usize,
) -> Result<FrameSummary, Box<dyn Error>> {
    let Some(batch_size) = std::num::NonZeroU32::new(8) else {
        return Err(io::Error::other("invalid zero batch size").into());
    };
    let batch_count = measured_frames.div_ceil(8);
    let mut batches = Vec::with_capacity(batch_count);
    let mut previous = telemetry(engine);
    for _ in 0..batch_count {
        let timing = engine.profile_frame_batch(scene, camera, config, batch_size)?;
        let current = timing_telemetry(&timing);
        batches.push(FrameSample::measured(
            timing.gpu_ns,
            timing.cpu_ns,
            timing.frame_ns,
            previous,
            current,
        )?);
        previous = current;
    }
    Ok(summarize(&batches, &mut Vec::with_capacity(batches.len()))?)
}

fn print_batch_summary(summary: FrameSummary) {
    println!("batch8_cpu_median_ns={}", summary.cpu_median_ns);
    println!("batch8_cpu_p95_ns={}", summary.cpu_p95_ns);
    println!("batch8_cpu_p99_ns={}", summary.cpu_p99_ns);
    println!("batch8_frame_median_ns={}", summary.frame_median_ns);
    println!("batch8_frame_p95_ns={}", summary.frame_p95_ns);
    println!("batch8_frame_p99_ns={}", summary.frame_p99_ns);
    println!("batch8_frame_p99_fps={:.2}", fps(summary.frame_p99_ns));
}

fn telemetry(engine: &Engine) -> CumulativeTelemetry {
    let value = engine.residency_counters();
    CumulativeTelemetry {
        allocation_events: value.allocation_events,
        upload_bytes: value.upload_bytes,
        resident_bytes: value.resident_bytes,
        stall_events: value.stall_events,
    }
}

fn timing_telemetry(timing: &molgfx::FrameTiming) -> CumulativeTelemetry {
    let value = timing.residency_counters();
    CumulativeTelemetry {
        allocation_events: value.allocation_events,
        upload_bytes: value.upload_bytes,
        resident_bytes: value.resident_bytes,
        stall_events: value.stall_events,
    }
}

fn aspect_ratio(config: ImageConfig) -> f32 {
    let Some(width) = num_traits::cast::<u32, f32>(config.width) else {
        return 0.0;
    };
    let Some(height) = num_traits::cast::<u32, f32>(config.height) else {
        return 0.0;
    };
    width / height
}

fn load_scene(path: &str, requested: Option<&str>) -> Result<Scene, Box<dyn Error>> {
    let compact_input = matches!(requested, None | Some("spacefill" | "points"));
    let options = pdbiox::ReadOptions::new()
        .mode(pdbiox::ParseMode::Recover)
        .only_first_model(true)
        .only_atomic_coords(compact_input);
    let (structure, diagnostics) =
        pdbiox::read_with_options(path, &options).map_err(|diagnostics| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("structure diagnostics: {diagnostics:?}"),
            )
        })?;
    if !diagnostics.is_empty() {
        eprintln!("structure recovered with diagnostics: {diagnostics:?}");
    }
    Ok(Scene::from_structure(&structure)?)
}

fn configure_representation(
    scene: &mut Scene,
    requested: Option<&str>,
    opacity: Option<&String>,
) -> Result<(), Box<dyn Error>> {
    let selection = scene.add_selection(AtomSelection::All);
    let represented = scene.represent(selection, representation(requested))?;
    let Some(value) = scene.representation_mut(represented) else {
        return Err(io::Error::other("new representation became stale").into());
    };
    if matches!(
        requested,
        Some(
            "vdw-surface"
                | "sas"
                | "sas-soft"
                | "ses"
                | "ses-contour"
                | "ses-dots"
                | "ses-mesh"
                | "ses-filled-contour"
                | "gaussian-surface",
        )
    ) {
        value.params.surface_kind = match requested {
            Some("vdw-surface") => SurfaceKind::VanDerWaals,
            Some("sas" | "sas-soft") => SurfaceKind::SolventAccessible,
            Some("gaussian-surface") => SurfaceKind::Gaussian,
            _ => SurfaceKind::SolventExcluded,
        };
        value.params.surface_style = match requested {
            Some("ses-contour") => SurfaceStyle::Contour,
            Some("ses-dots") => SurfaceStyle::Dots,
            Some("ses-mesh") => SurfaceStyle::Mesh,
            Some("ses-filled-contour") => SurfaceStyle::FilledContour,
            Some("sas-soft") => SurfaceStyle::SoftUnion,
            _ => SurfaceStyle::Solid,
        };
    }
    if let Some(opacity) = opacity {
        value.material.opacity = parse_opacity(opacity)?;
    }
    Ok(())
}

fn render_profile(name: Option<&str>) -> Result<RenderProfile, io::Error> {
    match name {
        None | Some("inspection") => Ok(RenderProfile::inspection()),
        Some("illustrative") => Ok(RenderProfile::illustrative()),
        Some("cinematic") => Ok(RenderProfile::cinematic()),
        Some(name) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown render profile {name}"),
        )),
    }
}

fn representation(value: Option<&str>) -> RepresentationKind {
    match value {
        Some("cartoon") => RepresentationKind::Cartoon,
        Some("trace") => RepresentationKind::Trace,
        Some("tube") => RepresentationKind::Tube,
        Some("ball-and-stick") => RepresentationKind::BallAndStick,
        Some("licorice") => RepresentationKind::Licorice,
        Some("lines") => RepresentationKind::Lines,
        Some("points") => RepresentationKind::Points,
        Some(
            "vdw-surface" | "sas" | "sas-soft" | "ses" | "ses-contour" | "ses-dots" | "ses-mesh"
            | "ses-filled-contour" | "gaussian-surface",
        ) => RepresentationKind::Surface,
        _ => RepresentationKind::Spacefill,
    }
}

fn parse_frames(value: Option<&String>) -> Result<usize, io::Error> {
    let Some(value) = value else {
        return Ok(DEFAULT_MEASURED_FRAMES);
    };
    let parsed = value.parse::<usize>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid frame count: {error}"),
        )
    })?;
    if parsed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame count must be positive",
        ));
    }
    Ok(parsed)
}

fn parse_opacity(value: &str) -> Result<f32, io::Error> {
    let opacity = value.parse::<f32>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid opacity: {error}"),
        )
    })?;
    if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "opacity must be finite and between zero and one",
        ));
    }
    Ok(opacity)
}

fn parse_dimension(value: Option<&String>, fallback: u32) -> Result<u32, io::Error> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    let dimension = value.parse::<u32>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid image dimension: {error}"),
        )
    })?;
    if dimension == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "image dimensions must be positive",
        ));
    }
    Ok(dimension)
}

fn fps(nanoseconds: u64) -> f64 {
    if nanoseconds == 0 {
        return f64::INFINITY;
    }
    1.0 / std::time::Duration::from_nanos(nanoseconds).as_secs_f64()
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
