//! Scene-linear 4K RGBA16F and deterministic `OpenEXR` publication export.

use molgfx::{
    AtomSelection, Camera, Engine, EngineConfig, ImageConfig, RenderMode, RepresentationKind, Scene,
};
use std::error::Error;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

#[path = "common/mod.rs"]
mod common;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let source = arguments
        .first()
        .map_or("benchmarks/scenes/3PTB.cif", String::as_str);
    let output = arguments
        .get(1)
        .map_or("target/visual-checks/publication-4k.exr", String::as_str);
    let width = parse_dimension(arguments.get(2), 3_840)?;
    let height = parse_dimension(arguments.get(3), 2_160)?;
    let mode = match arguments.get(4).map(String::as_str) {
        None | Some("realtime") => RenderMode::Realtime,
        Some("cinematic") => RenderMode::Cinematic,
        Some(value) => return Err(io::Error::other(format!("unknown render mode {value}")).into()),
    };

    let structure = common::read_structure(source)?;
    let mut scene = Scene::from_structure(&structure)?;
    let Some((owner, _)) = scene.structures().next() else {
        return Err(io::Error::other("HDR scene has no placed structure").into());
    };
    let secondary_structure = common::deposited_secondary_structure(source, &structure);
    if !secondary_structure.is_empty() {
        scene.apply_secondary_structure(owner, &secondary_structure)?;
    }
    let selection = scene.add_selection(AtomSelection::All);
    scene.represent(selection, RepresentationKind::BallAndStick)?;
    let camera = Camera::framing_aabb(&scene.world_aabb(), aspect(width, height)?);
    let mut engine = Engine::new(
        &EngineConfig {
            mode,
            ..EngineConfig::default()
        },
        None,
    )?;
    let image = engine.render_hdr_image(&scene, &camera, ImageConfig { width, height })?;
    let path = Path::new(output);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::with_capacity(64 * 1_024, file);
    image.write_exr(&mut writer)?;
    writer.flush()?;
    let encoded_bytes = writer.get_ref().metadata()?.len();
    println!(
        "wrote {output}: {width}x{height}, {} RGBA16F bytes, {} EXR bytes, mode={mode:?}",
        image.rgba16f().len(),
        encoded_bytes
    );
    Ok(())
}

fn aspect(width: u32, height: u32) -> Result<f32, io::Error> {
    let width = u16::try_from(width)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "image width is too large"))?;
    let height = u16::try_from(height)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "image height is too large"))?;
    Ok(f32::from(width) / f32::from(height))
}

fn parse_dimension(value: Option<&String>, fallback: u32) -> Result<u32, io::Error> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    let parsed = value.parse::<u32>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid image dimension: {error}"),
        )
    })?;
    if parsed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "image dimensions must be positive",
        ));
    }
    Ok(parsed)
}
