//! Declarative scene and style-edit benchmark driver.

use std::error::Error;
use std::io;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let Some(path) = std::env::args().nth(1) else {
        return Err(io::Error::other("usage: focus_profile STRUCTURE").into());
    };
    let structure = molframe::read(path).map_err(|diagnostics| {
        io::Error::new(io::ErrorKind::InvalidData, format!("{diagnostics:?}"))
    })?;

    let start = Instant::now();
    let mut scene = molgfx::Scene::from_structure(&structure)?;
    let cartoon = scene.add(molgfx::rep::cartoon(molgfx::sel::protein()))?;
    let ligand = scene.add(molgfx::rep::ball_and_stick(molgfx::sel::ligands()))?;
    let build = start.elapsed();

    let start = Instant::now();
    scene.transaction(|transaction| {
        transaction.set_opacity(cartoon, 0.35);
        transaction.set_opacity(ligand, 1.0);
        transaction.focus(molgfx::sel::ligands());
        Ok(())
    })?;
    let style_edit = start.elapsed();

    println!(
        "atoms={} scene_build_ns={} transaction_ns={} revision={}",
        structure.atom_count(),
        build.as_nanos(),
        style_edit.as_nanos(),
        scene.revision()
    );
    Ok(())
}
