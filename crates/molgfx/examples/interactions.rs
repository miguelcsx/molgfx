//! Renders deterministic caller-supplied generic relations and profiles them.

use molgfx::{
    AtomSelection, Camera, Engine, EngineConfig, ImageConfig, Relation, RelationBatch,
    RelationBatchHandle, RelationPattern, RelationStyle, RepresentationKind, RowDomain,
    RowEntityRef, Scene, SourceNamespace, SourceRows, SpatialAnchor,
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

fn add_interactions(scene: &mut Scene) -> Result<Vec<RelationBatchHandle>, Box<dyn Error>> {
    let owner = {
        let Some((owner, placed)) = scene.structures().next() else {
            return Err(io::Error::other("fixture structure is absent").into());
        };
        if placed.atoms.coords().slice().len() < 10 {
            return Err(io::Error::other("interaction fixture needs at least ten atoms").into());
        }
        owner
    };
    let pairs = [(0, 5), (2, 9), (4, 8), (1, 7), (3, 6)];
    let domain = RowDomain::Atoms(owner);
    let mut relations = Vec::with_capacity(pairs.len());
    for (start, end) in pairs {
        let start_index =
            u32::try_from(start).map_err(|_| io::Error::other("atom index overflow"))?;
        let end_index = u32::try_from(end).map_err(|_| io::Error::other("atom index overflow"))?;
        relations.push(Relation {
            start: SpatialAnchor::entity(RowEntityRef::new(domain, start_index))?,
            end: SpatialAnchor::entity(RowEntityRef::new(domain, end_index))?,
        });
    }
    let row_count =
        u32::try_from(relations.len()).map_err(|_| io::Error::other("relation count overflow"))?;
    let rows = SourceRows::ordered(SourceNamespace(0x696e_7465_7261_6374), row_count);
    let batch = RelationBatch::new(
        Arc::from(relations),
        rows,
        RelationStyle {
            width_pixels: 1.8,
            color: molgfx::Rgba8::opaque(96, 165, 250),
            opacity: 0.9,
            pattern: RelationPattern::Dashed,
            endpoint_insets_pixels: [0.0; 2],
            depth_behind_anchors: false,
        },
    )?;
    Ok(vec![scene.add_relation_batch(batch)?])
}

fn set_visible(
    scene: &mut Scene,
    handles: &[RelationBatchHandle],
    visible: bool,
) -> Result<(), Box<dyn Error>> {
    for &handle in handles {
        if !scene.set_domain_visible(RowDomain::Relations(handle), visible)? {
            return Err(io::Error::other("relation became stale").into());
        }
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

fn write_png(path: impl AsRef<Path>, image: &molgfx::Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
