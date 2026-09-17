//! A composed cinematic still: a populated scene staged in depth.
//!
//! This is caller-side composition, not invented biology — the engine places
//! exactly the structures it is given, at the transforms it is given. Real
//! depth in the scene is what lets the thin lens read: with a single molecule
//! centred in frame there is nothing behind or in front to fall out of focus,
//! and the image stays flat no matter how good the shading is.
//!
//! The documented route to an immersive picture is to put real biology in the
//! scene — structures, membranes, declared solvent, density — rather than to
//! dress up the fallback backdrop. So this stages several *different* entries
//! at different depths. The cast is chosen by *scale*: a chaperonin next to a
//! twelve-base duplex is honest biology but leaves a mostly empty frame, so
//! these players sit within roughly one order of magnitude of each other.
//!
//! Usage: `cargo run --example showcase --release --features semantic -- [out.png]`

use molgfx::{
    AtomSelection, BoundingSphere, Camera, ColorScheme, Engine, EngineConfig, Image, ImageConfig,
    Mat4, RenderMode, RenderProfile, RepresentationKind, Scene, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::path::Path;

/// Depth offsets and orientations for the staged copies, in units of the
/// structure's own radius. The hero sits at the origin; the rest fall away
/// from the focal plane on both sides.
/// 2560×1600 is a 1.6 frame; stating the ratio avoids an integer-to-float
/// cast that would lose precision for no benefit.
const ASPECT: f32 = 1.6;

/// One staged cast member: a file, and where it sits relative to the hero's
/// own radius.
struct Staged {
    path: &'static str,
    /// (right, up, forward) in hero radii, then yaw in radians.
    placement: (f32, f32, f32, f32),
}

const CAST: [Staged; 6] = [
    Staged {
        path: "benchmarks/scenes/4hhb.cif",
        placement: (0.0, 0.0, 0.0, 0.0),
    },
    Staged {
        path: "benchmarks/scenes/2RH1.cif",
        placement: (-1.02, 0.30, -0.62, 1.1),
    },
    Staged {
        path: "benchmarks/scenes/3PTB.cif",
        placement: (1.00, -0.40, 0.52, 2.3),
    },
    Staged {
        path: "benchmarks/scenes/1BNA.cif",
        placement: (-0.72, -0.66, 0.60, 0.4),
    },
    Staged {
        path: "benchmarks/scenes/1ubq.cif",
        placement: (0.82, 0.62, -1.05, 3.0),
    },
    Staged {
        path: "benchmarks/scenes/3PTB.cif",
        placement: (-1.18, 0.74, -1.45, 2.7),
    },
];

#[path = "common/mod.rs"]
mod common;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let output = arguments.first().map_or("showcase.png", String::as_str);

    let mut scene = Scene::new();
    let mut hero_bound = None;
    let mut radius = 1.0f32;
    for staged in &CAST {
        // A missing cast member leaves the rest of the scene standing.
        let Ok(structure) = common::read_structure(staged.path) else {
            continue;
        };
        let handle = scene.add_structure(&structure)?;
        let records = common::deposited_secondary_structure(staged.path, &structure);
        if !records.is_empty() {
            scene.apply_secondary_structure(handle, &records)?;
        }
        let (right, up, forward, yaw) = staged.placement;
        if hero_bound.is_none() {
            // The first entry sets the scale everything else is placed in.
            radius = scene.world_aabb().bounding_sphere().radius.max(1.0);
        }
        let Some(placed) = scene.structure_mut(handle) else {
            return Err("freshly placed structure became stale".into());
        };
        placed.model_to_world =
            Mat4::from_translation(Vec3::new(right * radius, up * radius, forward * radius))
                * Mat4::from_rotation_y(yaw);
        if hero_bound.is_none() {
            hero_bound = Some(placed.world_aabb().bounding_sphere());
        }
    }
    let Some(hero_bound) = hero_bound else {
        return Err("no cast member could be read".into());
    };

    // Cartoon over the whole cast: chain colour separates the players, and the
    // deposited secondary structure gives each its helices and sheets.
    let selection = scene.add_selection(AtomSelection::All);
    let represented = scene.represent(selection, RepresentationKind::Cartoon)?;
    let Some(representation) = scene.representation_mut(represented) else {
        return Err("new representation became stale".into());
    };
    representation.color = ColorScheme::ByChain;

    let config = ImageConfig {
        width: 2560,
        height: 1600,
    };
    // Frame the populated scene, not the hero alone: the cast is the picture.
    let staged_bound = scene.world_aabb().bounding_sphere();
    let mut camera = Camera::framing(
        &BoundingSphere {
            center: staged_bound.center.lerp(hero_bound.center, 0.35),
            radius: staged_bound.radius * 0.82,
        },
        ASPECT,
    );
    camera
        .projection
        .fit_near_far(camera.eye, &scene.world_aabb().bounding_sphere());

    let mut engine = Engine::new(
        &EngineConfig {
            mode: RenderMode::Cinematic,
            profile: RenderProfile::cinematic(),
            ..EngineConfig::default()
        },
        None,
    )?;
    let image = engine.render_image(&scene, &camera, config)?;
    write_png(output, &image)?;
    println!("composed {} staged entries into {output}", CAST.len());
    Ok(())
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
