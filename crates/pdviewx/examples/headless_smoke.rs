//! Opens the adapter, validates pipelines and optionally renders a structure to PNG.
//!
//! Usage: `cargo run --example headless_smoke -- structure.cif output.png [representation|layered] [width] [height] [opacity] [camera-distance-scale] [realtime|quality] [inspection|illustrative|cinematic]`

use pdviewx::{
    AtomSelection, BoundingSphere, Camera, ClipCap, ClipPlane, ClipSet, Engine, EngineConfig,
    Image, ImageConfig, RenderMode, RenderProfile, RepresentationKind, Scene, SurfaceKind,
    SurfaceStyle, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

#[path = "common/mod.rs"]
mod common;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = match arguments.get(7).map(String::as_str) {
        Some("quality") => RenderMode::Quality,
        _ => RenderMode::Realtime,
    };
    let profile = match arguments.get(8).map(String::as_str) {
        Some("illustrative") => RenderProfile::illustrative(),
        Some("cinematic") => RenderProfile::cinematic(),
        _ => RenderProfile::inspection(),
    };
    let mut engine = Engine::new(
        &EngineConfig {
            mode,
            profile,
            ..EngineConfig::default()
        },
        None,
    )?;
    println!("pdviewx adapter capabilities: {:?}", engine.capabilities());
    let (scene, camera, config) = match arguments.first() {
        Some(path) => molecular_scene(
            path,
            arguments.get(2).map(String::as_str),
            arguments.get(3),
            arguments.get(4),
            arguments.get(5),
            arguments.get(6),
        )?,
        None => empty_scene(),
    };
    let image = engine.render_image(&scene, &camera, config)?;
    println!("off-screen RGBA bytes: {}", image.pixels.len());
    println!(
        "center pick: {:?}",
        engine.pick(config.width / 2, config.height / 2)?
    );
    if let Some(path) = arguments.get(1) {
        write_png(path, &image)?;
        println!("wrote {path}");
    }
    Ok(())
}

fn empty_scene() -> (Scene, Camera, ImageConfig) {
    let camera = Camera::framing(
        &BoundingSphere {
            center: Vec3::ZERO,
            radius: 1.0,
        },
        1.0,
    );
    (
        Scene::new(),
        camera,
        ImageConfig {
            width: 16,
            height: 16,
        },
    )
}

/// Maps a caller's representation name onto its kind and surface settings.
fn resolve_kind(
    representation: Option<&str>,
) -> (
    RepresentationKind,
    Option<SurfaceKind>,
    Option<SurfaceStyle>,
) {
    match representation {
        Some("cartoon") => (RepresentationKind::Cartoon, None, None),
        Some("trace") => (RepresentationKind::Trace, None, None),
        Some("tube") => (RepresentationKind::Tube, None, None),
        Some("rocket") => (RepresentationKind::Rocket, None, None),
        Some("twister") => (RepresentationKind::Twister, None, None),
        Some("ball-and-stick" | "ball-and-stick-cutaway") => {
            (RepresentationKind::BallAndStick, None, None)
        }
        Some("licorice") => (RepresentationKind::Licorice, None, None),
        Some("lines") => (RepresentationKind::Lines, None, None),
        Some("beads") => (RepresentationKind::Beads, None, None),
        Some("points") => (RepresentationKind::Points, None, None),
        Some("vdw-surface") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::VanDerWaals),
            None,
        ),
        // The dotted van der Waals preset: the same uninflated boundary as
        // `vdw-surface`, sampled as a pixel-stable dot lattice.
        Some("vdw-dots") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::VanDerWaals),
            Some(SurfaceStyle::Dots),
        ),
        Some("sas") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::SolventAccessible),
            None,
        ),
        Some("ses-contour") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::SolventExcluded),
            Some(SurfaceStyle::Contour),
        ),
        Some("ses-dots") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::SolventExcluded),
            Some(SurfaceStyle::Dots),
        ),
        Some("ses" | "surface") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::SolventExcluded),
            None,
        ),
        _ => (RepresentationKind::Spacefill, None, None),
    }
}

fn molecular_scene(
    path: &str,
    representation: Option<&str>,
    width: Option<&String>,
    height: Option<&String>,
    opacity: Option<&String>,
    camera_distance_scale: Option<&String>,
) -> Result<(Scene, Camera, ImageConfig), Box<dyn Error>> {
    let structure = common::read_structure(path)?;
    let mut scene = Scene::from_structure(&structure)?;
    // Draw the cartoon the file actually describes: without the deposited
    // annotation every helix and strand renders as coil.
    let first_structure = scene.structures().next().map(|(handle, _)| handle);
    if let Some(handle) = first_structure {
        let records = common::deposited_secondary_structure(path, &structure);
        if !records.is_empty() {
            scene.apply_secondary_structure(handle, &records)?;
        }
    }
    let selection = scene.add_selection(AtomSelection::All);
    let opacity = opacity
        .map(|value| {
            value.parse::<f32>().map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid opacity: {error}"),
                )
            })
        })
        .transpose()?;
    if representation == Some("layered") {
        scene.represent(selection, RepresentationKind::Cartoon)?;
        let _ = add_representation(
            &mut scene,
            selection,
            RepresentationKind::Surface,
            opacity.or(Some(0.28)),
            Some(SurfaceKind::SolventExcluded),
            None,
        )?;
    } else {
        let (kind, surface_kind, surface_style) = resolve_kind(representation);
        let handle = add_representation(
            &mut scene,
            selection,
            kind,
            opacity,
            surface_kind,
            surface_style,
        )?;
        if representation == Some("ball-and-stick-cutaway") {
            let center = scene.world_aabb().center();
            let plane = ClipPlane::from_point_normal(center, -Vec3::Z)?;
            let Some(representation) = scene.representation_mut(handle) else {
                return Err(io::Error::other("new representation became stale").into());
            };
            representation.clipping = ClipSet::new(&[plane])?.with_cap(ClipCap::Solid);
        }
    }
    let width = parse_dimension(width, 1024)?;
    let height = parse_dimension(height, 768)?;
    let config = ImageConfig {
        width: u32::from(width),
        height: u32::from(height),
    };
    let bounds = scene.world_aabb();
    let mut camera = Camera::framing_aabb(&bounds, f32::from(width) / f32::from(height));
    if let Some(scale) = parse_camera_scale(camera_distance_scale)? {
        camera.eye = camera.target + (camera.eye - camera.target) * scale;
        camera
            .projection
            .fit_near_far(camera.eye, &bounds.bounding_sphere());
    }
    Ok((scene, camera, config))
}

fn parse_camera_scale(value: Option<&String>) -> Result<Option<f32>, io::Error> {
    let Some(value) = value else {
        return Ok(None);
    };
    let scale = value.parse::<f32>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid camera distance scale: {error}"),
        )
    })?;
    if !scale.is_finite() || !(0.1..=10.0).contains(&scale) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "camera distance scale must be finite and between 0.1 and 10",
        ));
    }
    Ok(Some(scale))
}

fn add_representation(
    scene: &mut Scene,
    selection: pdviewx::SelectionHandle,
    kind: RepresentationKind,
    opacity: Option<f32>,
    surface_kind: Option<SurfaceKind>,
    surface_style: Option<SurfaceStyle>,
) -> Result<pdviewx::RepresentationHandle, Box<dyn Error>> {
    let handle = scene.represent(selection, kind)?;
    if let Some(opacity) = opacity
        && (!opacity.is_finite() || !(0.0..=1.0).contains(&opacity))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "opacity must be finite and between zero and one",
        )
        .into());
    }
    let Some(representation) = scene.representation_mut(handle) else {
        return Err(io::Error::other("new representation became stale").into());
    };
    if let Some(opacity) = opacity {
        representation.material.opacity = opacity;
    }
    if let Some(surface_kind) = surface_kind {
        representation.params.surface_kind = surface_kind;
    }
    if let Some(surface_style) = surface_style {
        representation.params.surface_style = surface_style;
    }
    Ok(handle)
}

fn parse_dimension(value: Option<&String>, fallback: u16) -> Result<u16, io::Error> {
    let parsed = match value {
        Some(value) => value.parse::<u16>().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid image dimension: {error}"),
            )
        })?,
        None => fallback,
    };
    if parsed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "image dimensions must be positive",
        ));
    }
    Ok(parsed)
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
