//! Render independent eyes and common stereo layouts through the curated facade.
use molgfx::{Renderer, Scene, StereoLayout, molframe, rep, sel};
use std::{error::Error, io, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let input = PathBuf::from(
        arguments
            .next()
            .ok_or_else(|| io::Error::other("usage: stereo_image STRUCTURE OUTPUT_DIRECTORY"))?,
    );
    let output = PathBuf::from(
        arguments
            .next()
            .ok_or_else(|| io::Error::other("usage: stereo_image STRUCTURE OUTPUT_DIRECTORY"))?,
    );
    let name = input.to_string_lossy();
    let (structure, _) = molframe::read_bytes(
        std::fs::read(&input)?,
        Some(&name),
        &molframe::ReadOptions::new(),
    )?;
    let mut scene = Scene::from_structure(&structure)?;
    let _ = scene.add(rep::ball_and_stick(sel::all()))?;
    let camera = scene.frame(sel::all(), 4.0 / 3.0)?;
    let convergence = (camera.eye - camera.target).length();
    let mut renderer = Renderer::new()?;
    let images = renderer.render_stereo_images(
        &scene,
        &camera,
        (800, 600),
        convergence * 0.06,
        convergence,
    )?;
    std::fs::create_dir_all(&output)?;
    images.left.save(output.join("left.png"))?;
    images.right.save(output.join("right.png"))?;
    for (name, layout) in [
        ("side-by-side", StereoLayout::SideBySide),
        ("cross-eyed", StereoLayout::CrossEyed),
        ("red-cyan", StereoLayout::RedCyan),
        ("checkerboard", StereoLayout::Checkerboard),
    ] {
        images
            .compose(layout)?
            .save(output.join(format!("{name}.png")))?;
    }
    Ok(())
}
