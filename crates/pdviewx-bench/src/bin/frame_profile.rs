//! Whole-graph headless GPU benchmark using the production facade.

use pdviewx::{
    AtomSelection, Camera, Engine, EngineConfig, ImageConfig, RenderMode, RenderProfile,
    RepresentationKind, Scene, SurfaceKind, SurfaceStyle,
};
use pdviewx_bench::{FrameSample, summarize};
use std::error::Error;
use std::io;

const WARMUP_FRAMES: usize = 20;
const DEFAULT_MEASURED_FRAMES: usize = 120;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(path) = arguments.first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: frame_profile STRUCTURE [representation] [frames] [opacity] [realtime|quality] [width] [height] [inspection|illustrative|cinematic]",
        )
        .into());
    };
    let structure = pdbiox::read(path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let mut scene = Scene::from_structure(&structure)?;
    let selection = scene.add_selection(AtomSelection::All);
    let requested = arguments.get(1).map(String::as_str);
    let represented = scene.represent(selection, representation(requested))?;
    if matches!(
        requested,
        Some("vdw-surface" | "sas" | "ses" | "ses-contour" | "ses-dots")
    ) {
        let Some(representation) = scene.representation_mut(represented) else {
            return Err(io::Error::other("new representation became stale").into());
        };
        representation.params.surface_kind = match requested {
            Some("vdw-surface") => SurfaceKind::VanDerWaals,
            Some("sas") => SurfaceKind::SolventAccessible,
            _ => SurfaceKind::SolventExcluded,
        };
        representation.params.surface_style = match requested {
            Some("ses-contour") => SurfaceStyle::Contour,
            Some("ses-dots") => SurfaceStyle::Dots,
            _ => SurfaceStyle::Solid,
        };
    }
    if let Some(opacity) = arguments.get(3) {
        let opacity = parse_opacity(opacity)?;
        let Some(representation) = scene.representation_mut(represented) else {
            return Err(io::Error::other("new representation became stale").into());
        };
        representation.material.opacity = opacity;
    }
    let config = ImageConfig {
        width: parse_dimension(arguments.get(5), 1024)?,
        height: parse_dimension(arguments.get(6), 768)?,
    };
    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let measured_frames = parse_frames(arguments.get(2))?;
    let mode = match arguments.get(4).map(String::as_str) {
        Some("quality") => RenderMode::Quality,
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
    let mut samples = Vec::with_capacity(measured_frames);
    for _ in 0..measured_frames {
        let timing = engine.profile_frame(&scene, &camera, config)?;
        samples.push(FrameSample {
            gpu_ns: timing.gpu_ns,
            cpu_ns: timing.cpu_ns,
            ..FrameSample::default()
        });
    }
    let summary = summarize(&samples, &mut Vec::with_capacity(samples.len()))?;
    println!("atoms={atom_count}");
    println!("frames={measured_frames}");
    let profile_label = match profile_name {
        Some(name) => name,
        None => "inspection",
    };
    println!("profile={profile_label}");
    println!("gpu_median_ns={}", summary.gpu_median_ns);
    println!("gpu_p99_ns={}", summary.gpu_p99_ns);
    println!("cpu_median_ns={}", summary.cpu_median_ns);
    println!("gpu_median_fps={:.2}", fps(summary.gpu_median_ns));
    println!("gpu_p99_fps={:.2}", fps(summary.gpu_p99_ns));
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
        Some("vdw-surface" | "sas" | "ses" | "ses-contour" | "ses-dots") => {
            RepresentationKind::Surface
        }
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
    let bounded = u32::try_from(nanoseconds).map_or(u32::MAX, |value| value);
    1_000_000_000.0 / f64::from(bounded)
}
