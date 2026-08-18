//! Renders and profiles a caller-supplied topology-stable trajectory interval.
//!
//! The `synthetic` endpoint is an explicit visual-validation fixture, not an
//! inferred simulation: `trajectory [start.cif] [end.cif|synthetic] [prefix]`.

use pdviewx::{
    AtomSelection, Camera, Engine, EngineConfig, Image, ImageConfig, RepresentationKind, Scene,
    StructureHandle, TrajectoryFrame, TrajectorySegment,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/optics-depth-stack.cif", String::as_str);
    let endpoint_path = arguments.get(1).map_or("synthetic", String::as_str);
    let output_prefix = arguments
        .get(2)
        .map_or("target/visual-checks/trajectory", String::as_str);
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let endpoint = if endpoint_path == "synthetic" {
        None
    } else {
        Some(pdbiox::read(endpoint_path).map_err(|diagnostics| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("endpoint diagnostics: {diagnostics:?}"),
            )
        })?)
    };
    let mut scene = Scene::new();
    let handle = scene.add_structure(&structure)?;
    let selection = scene.add_selection(AtomSelection::All);
    scene.represent(selection, RepresentationKind::BallAndStick)?;
    scene.set_trajectory_segment(handle, trajectory(&scene, handle, endpoint.as_ref())?)?;

    let config = ImageConfig {
        width: 960,
        height: 720,
    };
    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    for (label, time) in [("start", 0.0), ("mid", 0.5), ("end", 1.0)] {
        scene.set_trajectory_time(handle, time)?;
        let image = engine.render_image(&scene, &camera, config)?;
        let path = format!("{output_prefix}-{label}.png");
        write_png(&path, &image)?;
        println!("wrote {path}");
    }
    if arguments.get(3).map(String::as_str) == Some("profile") {
        profile(&mut engine, &mut scene, handle, &camera, config)?;
    }
    Ok(())
}

fn trajectory(
    scene: &Scene,
    handle: StructureHandle,
    endpoint: Option<&pdbiox::Structure>,
) -> Result<TrajectorySegment, Box<dyn Error>> {
    let placed = scene
        .structure(handle)
        .ok_or_else(|| io::Error::other("placed structure is absent"))?;
    let start: Arc<[[f32; 3]]> = Arc::from(placed.atoms.coords().slice());
    let (end, endpoint_provenance): (Arc<[[f32; 3]]>, _) = match endpoint {
        Some(endpoint) => (
            Arc::from(
                endpoint
                    .model_positions(pdbiox::ModelIndex::new(0))
                    .ok_or_else(|| io::Error::other("endpoint has no first-model coordinates"))?,
            ),
            "caller:end-structure",
        ),
        None => (
            Arc::from(
                start
                    .iter()
                    .enumerate()
                    .map(|(index, position)| {
                        let phase_index = match u16::try_from(index) {
                            Ok(value) => f32::from(value),
                            Err(_) => f32::from(u16::MAX),
                        };
                        let phase = phase_index * 0.73;
                        [
                            position[0] + phase.sin() * 0.65,
                            position[1] + phase.cos() * 0.45,
                            position[2] + (phase * 0.47).sin() * 0.55,
                        ]
                    })
                    .collect::<Vec<_>>(),
            ),
            "synthetic:visual-validation",
        ),
    };
    let start = TrajectoryFrame::new(0, 0.0, start, "caller:start-structure")?;
    let end = TrajectoryFrame::new(1, 1.0, end, endpoint_provenance)?;
    Ok(TrajectorySegment::new(start, end, 0.0)?)
}

fn profile(
    engine: &mut Engine,
    scene: &mut Scene,
    handle: StructureHandle,
    camera: &Camera,
    config: ImageConfig,
) -> Result<(), Box<dyn Error>> {
    for index in 0..12 {
        scene.set_trajectory_time(handle, alternating_time(index))?;
        engine.profile_frame(scene, camera, config)?;
    }
    let mut gpu = Vec::with_capacity(40);
    let mut cpu = Vec::with_capacity(40);
    for index in 0..40 {
        scene.set_trajectory_time(handle, alternating_time(index))?;
        let timing = engine.profile_frame(scene, camera, config)?;
        gpu.push(timing.gpu_ns);
        cpu.push(timing.cpu_ns);
    }
    print_samples("trajectory frame advance", &mut gpu, &mut cpu)?;
    scene.clear_trajectory(handle)?;
    for _ in 0..12 {
        engine.profile_frame(scene, camera, config)?;
    }
    gpu.clear();
    cpu.clear();
    for _ in 0..40 {
        let timing = engine.profile_frame(scene, camera, config)?;
        gpu.push(timing.gpu_ns);
        cpu.push(timing.cpu_ns);
    }
    print_samples("parsed-coordinate baseline", &mut gpu, &mut cpu)
}

fn alternating_time(index: usize) -> f32 {
    if index.is_multiple_of(2) { 0.25 } else { 0.75 }
}

fn print_samples(label: &str, gpu: &mut [u64], cpu: &mut [u64]) -> Result<(), Box<dyn Error>> {
    gpu.sort_unstable();
    cpu.sort_unstable();
    let percentile = |samples: &[u64], numerator: usize| {
        samples
            .get((samples.len() * numerator).div_ceil(100).saturating_sub(1))
            .copied()
            .ok_or_else(|| io::Error::other("profiling produced no samples"))
    };
    let gpu_median = percentile(gpu, 50)?;
    let gpu_p99 = percentile(gpu, 99)?;
    let cpu_median = percentile(cpu, 50)?;
    println!(
        "{label}: GPU median {gpu_median} ns, GPU p99 {gpu_p99} ns, CPU median {cpu_median} ns"
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
