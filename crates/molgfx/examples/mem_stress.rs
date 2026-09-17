//! Headless memory stress: reuse one engine and render the same focus scene in
//! a tight loop, printing resident set size as it goes. If RSS plateaus after
//! warm-up, the engine holds no per-frame leak and any growth seen in the
//! windowed examples is the app redraw loop, not the renderer.
//!
//! Usage: `cargo run --release --example mem_stress --features semantic -- [frames] [structure.cif] [LIGAND]`

use molgfx::{
    AtomSelection, BoundingSphere, Camera, Engine, EngineConfig, ImageConfig, Scene, Vec3,
};
use molgfx_recipes::FocusScene;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let frames: usize = match args.first().map(|s| s.parse()) {
        Some(Ok(n)) => n,
        _ => 2000,
    };
    let path = args
        .get(1)
        .map_or("benchmarks/scenes/4hhb.cif", String::as_str);
    let ligand_name = args.get(2).map_or("HEM", String::as_str);

    let parsed = pdbiox::read(path).map_err(|d| format!("could not read {path}: {d:?}"))?;
    let structure = pdbiox::infer_bonds(
        &parsed,
        pdbiox::BondInference::default(),
        &pdbiox::ExecutionContext::default(),
    )
    .map_err(|d| format!("bond inference failed: {d:?}"))?
    .structure;

    let Some(residue) = structure
        .data()
        .residues()
        .find(|r| r.name() == Some(ligand_name))
    else {
        return Err(format!("component {ligand_name} is absent").into());
    };
    let atoms = residue.atoms().map(|a| a.index().get()).collect::<Vec<_>>();
    let points = atoms
        .iter()
        .filter_map(|index| {
            structure
                .positions()
                .get(usize::try_from(*index).ok()?)
                .copied()
                .map(Vec3::from_array)
        })
        .collect::<Vec<_>>();

    let mut scene = Scene::from_structure(&structure)?;
    let selection = scene.add_selection(AtomSelection::Sparse(atoms));
    // Building the focus scene allocates the SES surface field — the largest
    // scene-triggered buffer — so the loop exercises the real hot path.
    let _focus = scene.focus(selection)?;

    let sphere = BoundingSphere::from_points(&points);
    let frame = BoundingSphere {
        center: sphere.center,
        radius: sphere.radius * 1.6,
    };
    let config = ImageConfig {
        width: 1280,
        height: 960,
    };
    let width = u16::try_from(config.width).map_or(u16::MAX, |value| value);
    let height = u16::try_from(config.height).map_or(u16::MAX, |value| value);
    let camera = Camera::framing(&frame, f32::from(width) / f32::from(height));

    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    println!("stress: {frames} frames of {path} focusing {ligand_name}");
    report(0);
    for i in 1..=frames {
        // Reuse the same scene, camera and target shape on every iteration.
        let _image = engine.render_image(&scene, &camera, config)?;
        if i % 200 == 0 {
            report(i);
        }
    }
    Ok(())
}

/// Print the process resident set size in MiB via `ps`. Avoids an FFI
/// `getrusage` call so the example stays free of raw system calls.
fn report(frame: usize) {
    let pid = std::process::id().to_string();
    let rss_mib = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map(|kib| f64::from(u32::try_from(kib).map_or(u32::MAX, |value| value)) / 1024.0);
    match rss_mib {
        Some(mib) => println!("frame {frame:>6}: RSS {mib:8.1} MiB"),
        None => println!("frame {frame:>6}: RSS unavailable"),
    }
}
