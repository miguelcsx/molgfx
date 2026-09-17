//! Renders persistent annotations, markers and typed measurement guides.

use molgfx::{
    Annotation, AnnotationAnchor, AtomSelection, BackdropStyle, Camera, Engine, EngineConfig,
    Image, ImageConfig, MarkerShape, MarkerStyle, Measurement, PresentationEffect, RenderProfile,
    RepresentationKind, Rgba8, Scene, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

const IMAGE: ImageConfig = ImageConfig {
    width: 960,
    height: 720,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/optics-depth-stack.cif", String::as_str);
    let output = arguments
        .get(1)
        .map_or("target/visual-checks/annotations.png", String::as_str);
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let mut scene = Scene::from_structure(&structure)?;
    let selection = scene.add_selection(AtomSelection::All);
    scene.represent(selection, RepresentationKind::BallAndStick)?;
    add_labels(&mut scene)?;
    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let profile =
        RenderProfile::illustrative().with_effect(PresentationEffect::Backdrop(BackdropStyle {
            top: Rgba8::opaque(145, 157, 160),
            bottom: Rgba8::opaque(92, 108, 113),
            glow_color: Rgba8::opaque(204, 213, 211),
            glow_strength: 0.12,
        }));
    let mut engine = Engine::new(
        &EngineConfig {
            profile,
            ..EngineConfig::default()
        },
        None,
    )?;
    write_png(output, &engine.render_image(&scene, &camera, IMAGE)?)?;
    if arguments.get(2).map(String::as_str) == Some("profile") {
        profile_frames(&mut engine, &scene, &camera)?;
    }
    println!("wrote {output}");
    Ok(())
}

fn add_labels(scene: &mut Scene) -> Result<(), Box<dyn Error>> {
    let Some((owner, placed)) = scene.structures().next() else {
        return Err(io::Error::other("fixture structure is absent").into());
    };
    let positions = placed
        .atoms
        .coords()
        .slice()
        .iter()
        .copied()
        .map(Vec3::from_array)
        .collect::<Vec<_>>();
    if positions.len() < 9 {
        return Err(io::Error::other("annotation fixture needs nine atoms").into());
    }
    let front = 15.0;
    let note = Annotation::note(
        owner,
        AnnotationAnchor::world(Vec3::new(-2.0, 7.2, 8.0))?,
        "BINDING POCKET",
    )?
    .with_priority(30);
    scene.add_annotation(note)?;
    let hypothesis = Annotation::hypothesis(
        owner,
        AnnotationAnchor::world(Vec3::new(3.8, -7.2, front))?,
        "HYPOTHESIS",
    )?
    .with_priority(20);
    scene.add_annotation(hypothesis)?;
    let marker = Annotation::marker(
        owner,
        AnnotationAnchor::world(Vec3::new(0.0, 0.0, front))?,
        MarkerStyle {
            color: Rgba8::opaque(255, 188, 62),
            radius_pixels: 7.0,
            shape: MarkerShape::Crosshair,
        },
    )?
    .with_priority(40);
    scene.add_annotation(marker)?;
    let anchors = [world(positions[0])?, world(positions[2])?];
    scene.add_measurement(
        Measurement::distance(
            owner,
            anchors,
            positions[0].distance(positions[2]),
            "fixture:exact",
        )?
        .with_priority(10),
    )?;
    scene.add_measurement(
        Measurement::angle(
            owner,
            [
                world(positions[6])?,
                world(positions[7])?,
                world(positions[8])?,
            ],
            180.0,
            "fixture:exact",
        )?
        .with_priority(8),
    )?;
    Ok(())
}

fn world(position: Vec3) -> Result<AnnotationAnchor, molgfx::CoreError> {
    AnnotationAnchor::world(position)
}

fn profile_frames(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..16 {
        engine.profile_frame(scene, camera, IMAGE)?;
    }
    let mut gpu = Vec::with_capacity(80);
    let mut cpu = Vec::with_capacity(80);
    for _ in 0..80 {
        let timing = engine.profile_frame(scene, camera, IMAGE)?;
        gpu.push(timing.gpu_ns);
        cpu.push(timing.cpu_ns);
    }
    gpu.sort_unstable();
    cpu.sort_unstable();
    let median = *gpu
        .get(gpu.len() / 2)
        .ok_or_else(|| io::Error::other("no GPU samples"))?;
    let p99_index = (gpu.len() * 99).div_ceil(100).saturating_sub(1);
    let p99 = *gpu
        .get(p99_index)
        .ok_or_else(|| io::Error::other("no p99 sample"))?;
    let cpu_median = *cpu
        .get(cpu.len() / 2)
        .ok_or_else(|| io::Error::other("no CPU samples"))?;
    println!(
        "annotations: GPU median {median} ns ({:.2} FPS), p99 {p99} ns ({:.2} FPS), CPU median {cpu_median} ns",
        fps(median),
        fps(p99),
    );
    Ok(())
}

fn fps(nanoseconds: u64) -> f64 {
    1.0 / std::time::Duration::from_nanos(nanoseconds.max(1)).as_secs_f64()
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
