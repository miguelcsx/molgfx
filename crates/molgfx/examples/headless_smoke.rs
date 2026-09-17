//! Opens the adapter, validates pipelines and optionally renders a structure to PNG.
//!
//! Usage: `cargo run --example headless_smoke -- structure.cif output.png [representation|layered|glycan|glycan-closeup] [width] [height] [opacity] [camera-distance-scale] [realtime|cinematic] [inspection|illustrative|cinematic]`

use molgfx::{
    AtomSelection, BondTopologyFrame, BondTopologySegment, BoundingSphere, Camera, ClipCap,
    ClipPlane, ClipSet, ColorScheme, Engine, EngineConfig, GuideStyle, IllustrationStyle, Image,
    ImageConfig, PlaybackMode, PresentationEffect, RenderMode, RenderProfile, RepresentationKind,
    ScalarFieldSemantics, Scene, StructureHandle, SurfaceKind, SurfaceStyle, TimeWarp, Timeline,
    TopologyBond, TrajectoryFrame, TrajectorySegment, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

#[path = "common/mod.rs"]
mod common;
#[path = "common/glycan.rs"]
mod glycan;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = match arguments.get(7).map(String::as_str) {
        Some("cinematic") => RenderMode::Cinematic,
        _ => RenderMode::Realtime,
    };
    let profile = match arguments.get(8).map(String::as_str) {
        Some("illustrative") => RenderProfile::illustrative(),
        Some("cinematic") => RenderProfile::cinematic(),
        Some("blueprint") => RenderProfile::inspection().with_effect(
            PresentationEffect::Illustration(IllustrationStyle {
                silhouette_strength: 0.9,
                cavity_strength: 0.3,
                depth_cue_strength: 0.0,
                posterize_levels: 4.0,
                motion_persistence: 0.0,
                outline_width: 3.0,
            }),
        ),
        Some("trails") => RenderProfile::inspection().with_effect(
            PresentationEffect::Illustration(IllustrationStyle {
                motion_persistence: 0.9,
                ..IllustrationStyle::default()
            }),
        ),
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
    println!("molgfx adapter capabilities: {:?}", engine.capabilities());
    let (mut scene, camera, config) = match arguments.first() {
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
    configure_motion(&arguments, &mut scene, &mut engine, &camera, config)?;
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

fn configure_motion(
    arguments: &[String],
    scene: &mut Scene,
    engine: &mut Engine,
    camera: &Camera,
    config: ImageConfig,
) -> Result<(), Box<dyn Error>> {
    let handle = scene.structures().next().map(|(handle, _)| handle);
    if let (Some(argument), Some(handle)) = (arguments.get(9), handle)
        && let Ok(length) = argument.parse::<f32>()
    {
        scene.set_bond_break_length(handle, length)?;
        println!("bond break length: {length}");
    }
    // Trajectory-driven modes reuse one synthesized rotational displacement.
    match (arguments.get(10).map(String::as_str), handle) {
        (Some("porcupine"), Some(handle)) => {
            let count = swirl_trajectory(scene, handle)?;
            let indices: Vec<u32> = (0..count).step_by(12).collect();
            let style = GuideStyle {
                color: molgfx::Rgba8::opaque(220, 40, 40),
                width_pixels: 2.6,
                arrow_pixels: 12.0,
                ..GuideStyle::default()
            };
            let arrows = scene.add_trajectory_vectors(handle, &indices, 3.0, 0.25, style)?;
            println!("porcupine arrows: {}", arrows.len());
        }
        (Some("rmsf"), Some(handle)) => {
            swirl_trajectory(scene, handle)?;
            let property = scene.trajectory_displacement_property(
                handle,
                "per-frame motion",
                ScalarFieldSemantics::UncalibratedRank,
            )?;
            let property_handle = scene.add_atom_property(property)?;
            let representation = scene.representations().next().map(|(handle, _)| handle);
            let color = scene
                .atom_property(property_handle)
                .map(|value| ColorScheme::property(property_handle, value));
            if let (Some(color), Some(representation)) = (color, representation)
                && let Some(view) = scene.representation_mut(representation)
            {
                view.color = color;
                println!("rmsf coloring applied");
            }
        }
        (Some("reaction"), Some(handle)) => configure_reaction(arguments, scene, handle)?,
        _ => {}
    }
    // Motion trails are temporal: seed the history with a warm-up frame at the
    // interval start, then advance the trajectory so the final frame carries
    // real motion for the persistence blend to smear.
    if arguments.get(8).map(String::as_str) == Some("trails")
        && let Some(handle) = handle
    {
        swirl_trajectory(scene, handle)?;
        let warp = TimeWarp::new(0.0, 0.0, 1.0, [0.0, 1.0], PlaybackMode::Clamp)?;
        let mut timeline = Timeline::new();
        timeline.bind_trajectory(scene, handle, warp)?;
        for step in 0u8..7 {
            timeline.apply(scene, f64::from(step) * 0.1)?;
            let _warm_up = engine.render_image(scene, camera, config)?;
        }
        timeline.apply(scene, 0.75)?;
        println!("motion trails: warmed up 7 frames of continuous motion");
    }
    Ok(())
}

fn configure_reaction(
    arguments: &[String],
    scene: &mut Scene,
    handle: StructureHandle,
) -> Result<(), Box<dyn Error>> {
    let (atom_count, mut start) = match scene.structure(handle) {
        Some(placed) => {
            let mut bonds = Vec::with_capacity(placed.structure.data().bonds.len());
            for bond in placed.structure.data().bonds.iter() {
                let atom_a = u32::try_from(bond.atom_a.as_usize())
                    .map_err(|_| io::Error::other("bond atom index exceeds u32"))?;
                let atom_b = u32::try_from(bond.atom_b.as_usize())
                    .map_err(|_| io::Error::other("bond atom index exceeds u32"))?;
                bonds.push(TopologyBond::new(
                    atom_a,
                    atom_b,
                    bond.order == pdbiox::BondOrder::Aromatic,
                )?);
            }
            (placed.atoms.len(), bonds)
        }
        None => return Ok(()),
    };
    start.sort_unstable_by_key(|bond| bond.atoms());
    start.dedup_by_key(|bond| bond.atoms());
    let end = start
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, bond)| (index % 4 != 0).then_some(bond))
        .collect::<Vec<_>>();
    let start = BondTopologyFrame::new(0, 0.0, atom_count, Arc::from(start), "reaction:start")?;
    let end = BondTopologyFrame::new(1, 1.0, atom_count, Arc::from(end), "reaction:end")?;
    scene.set_bond_topology_segment(handle, BondTopologySegment::new(start, end, 0.0)?)?;
    let sample = match arguments
        .get(11)
        .and_then(|value| value.parse::<f64>().ok())
    {
        Some(sample) => sample,
        None => 0.75,
    }
    .clamp(0.0, 1.0);
    let warp = TimeWarp::new(0.0, 0.0, 1.0, [0.0, 1.0], PlaybackMode::Clamp)?;
    let mut timeline = Timeline::new();
    timeline.bind_bond_topology(scene, handle, warp)?;
    timeline.apply(scene, sample)?;
    println!("dynamic topology sample: {sample:.3}");
    Ok(())
}

/// Installs a two-frame rotational displacement about the scene centroid and
/// returns the atom count, so porcupine and RMSF modes share one motion field.
fn swirl_trajectory(scene: &mut Scene, handle: StructureHandle) -> Result<u32, Box<dyn Error>> {
    let bounds = scene.world_aabb();
    let centroid = (bounds.min + bounds.max) * 0.5;
    let base: Vec<[f32; 3]> = match scene.structures().next() {
        Some((_, placed)) => placed.atoms.coords().slice().to_vec(),
        None => Vec::new(),
    };
    let (sin, cos) = 0.18_f32.sin_cos();
    let end: Vec<[f32; 3]> = base
        .iter()
        .map(|point| {
            let local = Vec3::from_array(*point) - centroid;
            let rotated = Vec3::new(
                local.x * cos - local.y * sin,
                local.x * sin + local.y * cos,
                local.z,
            );
            (rotated + centroid).to_array()
        })
        .collect();
    let count = u32::try_from(base.len()).map_or(0, |value| value);
    let start = TrajectoryFrame::new(0, 0.0, Arc::from(base), "swirl:start")?;
    let finish = TrajectoryFrame::new(1, 1.0, Arc::from(end), "swirl:end")?;
    scene.set_trajectory_segment(handle, TrajectorySegment::new(start, finish, 0.0)?)?;
    Ok(count)
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
        Some("paper-chain") => (RepresentationKind::PaperChain, None, None),
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
        Some("sas-soft") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::SolventAccessible),
            Some(SurfaceStyle::SoftUnion),
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
        Some("ses-mesh") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::SolventExcluded),
            Some(SurfaceStyle::Mesh),
        ),
        Some("ses-filled-contour") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::SolventExcluded),
            Some(SurfaceStyle::FilledContour),
        ),
        Some("gaussian-surface") => (
            RepresentationKind::Surface,
            Some(SurfaceKind::Gaussian),
            None,
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
    // Twister and PaperChain describe sugars, and a sugar in isolation is a few
    // rings floating in space. Drawn over the protein they came off, they read
    // as what they are, so these two modes compose the scene the pair is for.
    if matches!(representation, Some("glycan" | "glycan-closeup")) {
        glycan::compose_glycan_scene(&mut scene, &structure)?;
    } else if representation == Some("layered") {
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
    let bounds = match representation {
        Some("glycan-closeup") => glycan::sugar_bounds(&structure),
        _ => scene.world_aabb(),
    };
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
    selection: molgfx::SelectionHandle,
    kind: RepresentationKind,
    opacity: Option<f32>,
    surface_kind: Option<SurfaceKind>,
    surface_style: Option<SurfaceStyle>,
) -> Result<molgfx::RepresentationHandle, Box<dyn Error>> {
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
