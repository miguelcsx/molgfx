//! Streams a physically shaded 4K orbit of a real molecular assembly to PNG.
//!
//! Usage: `cargo run --release --example cinematic_4k -- [output-dir] [frames]`

use molgfx::{
    BackdropStyle, BloomStyle, BoundingSphere, Camera, ColorScheme, DisplayTransform, Engine,
    EngineConfig, Image, ImageConfig, LightingEnvironment, Material, PresentationEffect, Quat,
    RenderMode, RenderProfile, RepresentationKind, Rgba8, Scene, Select, SequenceConfig, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

#[path = "common/mod.rs"]
mod common;

const SOURCE: &str = "benchmarks/scenes/4hhb.cif";
const WIDTH: u32 = 3_840;
const HEIGHT: u32 = 2_160;
const ASPECT: f32 = 16.0 / 9.0;
const DEFAULT_FRAMES: u16 = 48;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let output = arguments.first().map_or_else(
        || PathBuf::from("target/visual-checks/cinematic-4k"),
        PathBuf::from,
    );
    let frames = parse_frames(arguments.get(1))?;
    std::fs::create_dir_all(&output)?;

    let structure = common::read_structure(SOURCE)?;
    let mut scene = Scene::from_structure(&structure)?;
    let owner = scene
        .structures()
        .next()
        .map(|(handle, _)| handle)
        .ok_or_else(|| io::Error::other("cinematic scene has no structure"))?;
    let secondary = common::deposited_secondary_structure(SOURCE, &structure);
    if !secondary.is_empty() {
        scene.apply_secondary_structure(owner, &secondary)?;
    }
    add_realistic_layers(&mut scene)?;

    let bounds = scene.world_aabb().bounding_sphere();
    let base = Camera::framing_aabb(&scene.world_aabb(), ASPECT);
    let base_offset = (base.eye - base.target) * 1.24;
    let profile = RenderProfile::illustrative()
        .with_effect(PresentationEffect::Lighting(
            LightingEnvironment::documentary(),
        ))
        .with_effect(PresentationEffect::Display(DisplayTransform::cinematic()))
        .with_effect(PresentationEffect::Bloom(BloomStyle {
            threshold: 1.1,
            intensity: 0.08,
            radius: 2.0,
        }))
        .with_effect(PresentationEffect::Backdrop(BackdropStyle {
            top: Rgba8::opaque(22, 31, 47),
            bottom: Rgba8::opaque(4, 7, 13),
            glow_color: Rgba8::opaque(24, 56, 76),
            glow_strength: 0.14,
        }));
    let mut engine = Engine::new(
        &EngineConfig {
            mode: RenderMode::Realtime,
            profile,
            ..EngineConfig::default()
        },
        None,
    )?;
    let config = ImageConfig {
        width: WIDTH,
        height: HEIGHT,
    };
    let sequence_config = SequenceConfig::at_fps(config, 30, 3)?;
    let mut sequence = engine.sequence(sequence_config)?;

    for frame in 0..frames {
        let denominator = f32::from(frames.saturating_sub(1).max(1));
        let time = f32::from(frame) / denominator;
        let camera = orbit_camera(base, base_offset, &bounds, time);
        sequence.submit(&mut engine, &scene, &camera, u64::from(frame))?;
        if sequence.pending() == usize::from(sequence_config.max_in_flight) {
            for completed in sequence.finish(&mut engine)? {
                write_png(
                    output.join(format!("cinematic-{:03}.png", completed.ticket.timestamp)),
                    &completed.image,
                )?;
            }
            sequence = engine.sequence(sequence_config)?;
        }
    }
    for completed in sequence.finish(&mut engine)? {
        write_png(
            output.join(format!("cinematic-{:03}.png", completed.ticket.timestamp)),
            &completed.image,
        )?;
    }
    println!(
        "wrote {frames} physically shaded {WIDTH}x{HEIGHT} frames to {}",
        output.display()
    );
    Ok(())
}

fn orbit_camera(mut camera: Camera, offset: Vec3, bounds: &BoundingSphere, time: f32) -> Camera {
    let eased = time * time * (3.0 - 2.0 * time);
    let orbit = Quat::from_rotation_y(0.34 * eased)
        * Quat::from_rotation_x((time * std::f32::consts::TAU).sin() * 0.045);
    camera.eye = camera.target + orbit * offset;
    camera.up = orbit * camera.up;
    camera.projection.fit_near_far(camera.eye, bounds);
    camera
}

fn add_realistic_layers(scene: &mut Scene) -> Result<(), Box<dyn Error>> {
    let polymer = scene.select(Select::polymer())?;
    let cartoon = scene.represent(polymer, RepresentationKind::Cartoon)?;
    let view = scene
        .representation_mut(cartoon)
        .ok_or_else(|| io::Error::other("cartoon representation became stale"))?;
    view.color = ColorScheme::ByChain;
    view.params.ribbon_width = 1.22;
    view.material = Material::anisotropic_ribbon(0.62);
    view.material.roughness = 0.34;
    view.material.specular = 0.32;
    view.order = 1;

    let ligands = scene.select(Select::ligands())?;
    let ligand = scene.represent(ligands, RepresentationKind::BallAndStick)?;
    let view = scene
        .representation_mut(ligand)
        .ok_or_else(|| io::Error::other("ligand representation became stale"))?;
    view.params.radius_scale = 0.38;
    view.params.bond_radius = 0.20;
    view.material = Material::principled(0.08);
    view.material.roughness = 0.24;
    view.material.specular = 0.46;
    view.order = 3;

    Ok(())
}

fn parse_frames(value: Option<&String>) -> Result<u16, io::Error> {
    let Some(value) = value else {
        return Ok(DEFAULT_FRAMES);
    };
    let frames = value.parse::<u16>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid frame count: {error}"),
        )
    })?;
    if frames < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least two frames are required",
        ));
    }
    Ok(frames)
}

#[cfg(test)]
#[path = "cinematic_4k/tests.rs"]
mod tests;

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
