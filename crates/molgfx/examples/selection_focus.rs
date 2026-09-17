//! Compares camera-target and selection-tracked thin-lens focus.
//!
//! Usage: `cargo run --example selection_focus --release -- [scene.cif] [camera.png] [selection.png] [profile]`

use molgfx::{
    AtomSelection, Camera, DepthOfField, Engine, EngineConfig, FocusTarget, Image, ImageConfig,
    PresentationEffect, RenderProfile, RepresentationKind, Scene,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

const IMAGE: ImageConfig = ImageConfig {
    width: 1_000,
    height: 750,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let input = arguments
        .first()
        .map_or("benchmarks/scenes/optics-depth-stack.cif", String::as_str);
    let camera_output = arguments.get(1).map_or(
        "target/visual-checks/focus-camera-target.png",
        String::as_str,
    );
    let selection_output = arguments.get(2).map_or(
        "target/visual-checks/focus-selection-tracked.png",
        String::as_str,
    );
    let structure = pdbiox::read(input).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let mut scene = Scene::new();
    let structure = scene.add_structure(&structure)?;
    let all = scene.add_structure_selection(structure, AtomSelection::All)?;
    scene.represent(all, RepresentationKind::Spacefill)?;
    let front = scene.add_structure_selection(structure, AtomSelection::Range(0..9))?;
    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let mut engine = Engine::new(
        &EngineConfig {
            profile: optics(FocusTarget::CameraTarget),
            ..EngineConfig::default()
        },
        None,
    )?;
    write_png(camera_output, &engine.render_image(&scene, &camera, IMAGE)?)?;
    engine.set_render_profile(optics(FocusTarget::Selection(front)))?;
    write_png(
        selection_output,
        &engine.render_image(&scene, &camera, IMAGE)?,
    )?;
    if arguments.get(3).map(String::as_str) == Some("profile") {
        profile(&mut engine, &scene, &camera)?;
    }
    println!("wrote {camera_output}");
    println!("wrote {selection_output}");
    Ok(())
}

fn optics(focus: FocusTarget) -> RenderProfile {
    let mut lens = DepthOfField::cinematic();
    lens.f_number = 1.8;
    lens.max_blur_pixels = 14.0;
    lens.focus = focus;
    RenderProfile::cinematic().with_effect(PresentationEffect::DepthOfField(lens))
}

fn profile(engine: &mut Engine, scene: &Scene, camera: &Camera) -> Result<(), Box<dyn Error>> {
    for _ in 0..24 {
        engine.profile_frame(scene, camera, IMAGE)?;
    }
    let mut gpu = Vec::with_capacity(60);
    let mut cpu = Vec::with_capacity(60);
    for _ in 0..60 {
        let timing = engine.profile_frame(scene, camera, IMAGE)?;
        gpu.push(timing.gpu_ns);
        cpu.push(timing.cpu_ns);
    }
    gpu.sort_unstable();
    cpu.sort_unstable();
    let median = |samples: &[u64]| samples.get(samples.len() / 2).copied();
    let p99 = |samples: &[u64]| {
        samples
            .get((samples.len() * 99).div_ceil(100).saturating_sub(1))
            .copied()
    };
    let (Some(gpu_median), Some(gpu_p99), Some(cpu_median)) =
        (median(&gpu), p99(&gpu), median(&cpu))
    else {
        return Err(io::Error::other("profiling produced no samples").into());
    };
    println!(
        "selection focus: GPU median {gpu_median} ns, GPU p99 {gpu_p99} ns, CPU median {cpu_median} ns"
    );
    Ok(())
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
