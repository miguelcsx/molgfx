//! Renders deterministic caller-supplied interaction glyphs and profiles them.

use pdviewx::{
    AtomSelection, Camera, Engine, EngineConfig, EntityKind, EntityRef, ImageConfig,
    InteractionAnchor, InteractionDirection, InteractionEdge, InteractionGeometry,
    InteractionHandle, InteractionKind, RepresentationKind, Scene, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/optics-depth-stack.cif", String::as_str);
    let output = arguments
        .get(1)
        .map_or("target/visual-checks/interactions.png", String::as_str);
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let mut scene = Scene::from_structure(&structure)?;
    let selection = scene.add_selection(AtomSelection::All);
    scene.represent(selection, RepresentationKind::BallAndStick)?;
    let interactions = add_interactions(&mut scene)?;
    let config = ImageConfig {
        width: 960,
        height: 720,
    };
    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    let image = engine.render_image(&scene, &camera, config)?;
    write_png(output, &image)?;
    if arguments.get(2).map(String::as_str) == Some("profile") {
        set_visible(&mut scene, &interactions, false)?;
        profile(
            &mut engine,
            &scene,
            &camera,
            config,
            "ball-and-stick baseline",
        )?;
        set_visible(&mut scene, &interactions, true)?;
        profile(&mut engine, &scene, &camera, config, "interaction glyphs")?;
    }
    println!("wrote {output}");
    Ok(())
}

fn add_interactions(scene: &mut Scene) -> Result<Vec<InteractionHandle>, Box<dyn Error>> {
    let (owner, positions) = {
        let Some((owner, placed)) = scene.structures().next() else {
            return Err(io::Error::other("fixture structure is absent").into());
        };
        let coordinates = placed.atoms.coords().slice();
        if coordinates.len() < 10 {
            return Err(io::Error::other("interaction fixture needs at least ten atoms").into());
        }
        let positions = coordinates[..10]
            .iter()
            .copied()
            .map(Vec3::from_array)
            .collect::<Vec<_>>();
        (owner, positions)
    };
    let pairs = [(0, 5), (2, 9), (4, 8), (1, 7), (3, 6)];
    let kinds = [
        InteractionKind::HydrogenBond,
        InteractionKind::SaltBridge,
        InteractionKind::PiStacking,
        InteractionKind::Hydrophobic,
        InteractionKind::MetalCoordination,
    ];
    let mut edges = Vec::with_capacity(kinds.len());
    for (ordinal, ((start, end), kind)) in pairs.into_iter().zip(kinds).enumerate() {
        let start_position = positions[start];
        let end_position = positions[end];
        let start_index =
            u32::try_from(start).map_err(|_| io::Error::other("atom index overflow"))?;
        let end_index = u32::try_from(end).map_err(|_| io::Error::other("atom index overflow"))?;
        let start = anchor(owner, start_index, start_position)?;
        let end = anchor(owner, end_index, end_position)?;
        let geometry = InteractionGeometry::new(start_position.distance(end_position), None)?;
        let direction = if ordinal % 2 == 0 {
            InteractionDirection::Forward
        } else {
            InteractionDirection::Undirected
        };
        let ordinal = u16::try_from(ordinal)
            .map(f32::from)
            .map_err(|_| io::Error::other("interaction ordinal overflow"))?;
        let edge = InteractionEdge::new(
            owner,
            start,
            end,
            kind,
            geometry,
            "pdviewx deterministic interaction fixture",
        )?
        .with_direction(direction)
        .with_occupancy(0.55 + ordinal * 0.1)?
        .with_normalized_strength(0.25 + ordinal * 0.15)?;
        edges.push(scene.add_interaction(edge)?);
    }
    Ok(edges)
}

fn anchor(
    structure: pdviewx::StructureHandle,
    index: u32,
    position: Vec3,
) -> Result<InteractionAnchor, pdviewx::CoreError> {
    InteractionAnchor::entity(
        position,
        EntityRef {
            structure,
            kind: EntityKind::Atom,
            index,
        },
    )
}

fn set_visible(
    scene: &mut Scene,
    handles: &[InteractionHandle],
    visible: bool,
) -> Result<(), io::Error> {
    for &handle in handles {
        scene
            .interaction_mut(handle)
            .ok_or_else(|| io::Error::other("interaction became stale"))?
            .set_visible(visible);
    }
    Ok(())
}

fn profile(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
    config: ImageConfig,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..12 {
        engine.profile_frame(scene, camera, config)?;
    }
    let mut samples = Vec::with_capacity(40);
    for _ in 0..40 {
        samples.push(engine.profile_frame(scene, camera, config)?.gpu_ns);
    }
    samples.sort_unstable();
    let median = samples
        .get(samples.len() / 2)
        .copied()
        .ok_or_else(|| io::Error::other("profiling produced no samples"))?;
    let p99 = samples
        .get((samples.len() * 99).div_ceil(100).saturating_sub(1))
        .copied()
        .ok_or_else(|| io::Error::other("profiling percentile is absent"))?;
    let median = u32::try_from(median).map_or(u32::MAX, |value| value);
    let p99 = u32::try_from(p99).map_or(u32::MAX, |value| value);
    println!(
        "{label}: GPU median {median} ns ({:.2} FPS), p99 {p99} ns ({:.2} FPS)",
        1_000_000_000.0 / f64::from(median.max(1)),
        1_000_000_000.0 / f64::from(p99.max(1)),
    );
    Ok(())
}

fn write_png(path: impl AsRef<Path>, image: &pdviewx::Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
