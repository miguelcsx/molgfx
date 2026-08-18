//! Proves that a rendered molecular pixel resolves through the ID buffers.
//!
//! Usage: `picking_smoke [structure.cif] [output.png]`

use pdviewx::{
    AtomSelection, Camera, Engine, EngineConfig, ImageConfig, RepresentationKind, Scene,
};
use std::error::Error;
use std::fs;
use std::io;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/1BNA.cif", String::as_str);
    let output_path = arguments
        .get(1)
        .map_or("target/visual-checks/picking.png", String::as_str);
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let mut scene = Scene::from_structure(&structure)?;
    let selection = scene.add_selection(AtomSelection::All);
    scene.represent(selection, RepresentationKind::BallAndStick)?;
    let camera = Camera::framing_aabb(&scene.world_aabb(), 1.0);
    let config = ImageConfig {
        width: 256,
        height: 256,
    };
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    let image = engine.render_image(&scene, &camera, config)?;
    let background = image
        .pixels
        .get(..4)
        .ok_or_else(|| io::Error::other("rendered image has no first pixel"))?;
    let Some((pixel_index, pick)) = image
        .pixels
        .chunks_exact(4)
        .enumerate()
        .filter(|(_, pixel)| {
            pixel[..3]
                .iter()
                .zip(background)
                .any(|(value, base)| value.abs_diff(*base) > 4)
        })
        .find_map(|(index, _)| {
            let x = u32::try_from(index % usize::try_from(config.width).ok()?).ok()?;
            let y = u32::try_from(index / usize::try_from(config.width).ok()?).ok()?;
            engine.pick(x, y).ok().flatten().map(|pick| (index, pick))
        })
    else {
        return Err(io::Error::other("no rendered molecular pixel resolved to an entity").into());
    };
    let x = pixel_index % usize::try_from(config.width)?;
    let y = pixel_index / usize::try_from(config.width)?;
    fs::write(output_path, image.png_bytes()?)?;
    println!("pick=true");
    println!("pixel={x},{y}");
    println!("entity={:?}", pick.entity);
    println!("selection={:?}", pick.selection);
    println!("wrote {output_path}");
    Ok(())
}
