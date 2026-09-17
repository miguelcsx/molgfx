//! Pocket surface with many deterministic candidate ligand poses in licorice.
//!
//! Usage: `cargo run --release --example docking_swarm --features semantic --
//! [structure] [ligand] [candidate-count] [output.png] [profile-frames]
//! [opacity] [realtime|cinematic] [surface|instances-only|surface-only]`

use molgfx::{
    AnalyticCapsule, AnalyticSphere, AnalyticTemplate, AtomSelection, BoundingSphere, Camera,
    ColorScheme, Engine, EngineConfig, Image, ImageConfig, InstanceBatch, InstanceStyle, Quat,
    RenderMode, RepresentationKind, Rgba8, RigidInstance, Scene, Select, SourceNamespace,
    SourceRows, SurfaceKind, SurfaceStyle, SurfaceZoneScene, SurfaceZoneStyle, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

const IMAGE: ImageConfig = ImageConfig {
    width: 1024,
    height: 768,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let path = args
        .first()
        .map_or("benchmarks/scenes/3PTB.cif", String::as_str);
    let ligand_name = args.get(1).map_or("BEN", String::as_str);
    let candidates = parse_usize(args.get(2), 400, "candidate count")?;
    let output = args
        .get(3)
        .map_or("target/visual-checks/docking-swarm.png", String::as_str);
    let profile_frames = parse_usize(args.get(4), 0, "profile frames")?;
    let opacity = parse_opacity(args.get(5))?;
    let mode = match args.get(6).map(String::as_str) {
        None | Some("realtime") => RenderMode::Realtime,
        Some("cinematic") => RenderMode::Cinematic,
        Some(value) => return Err(io::Error::other(format!("unknown render mode {value}")).into()),
    };
    let (surface_enabled, instances_enabled) = match args.get(7).map(String::as_str) {
        None | Some("surface") => (true, true),
        Some("instances-only") => (false, true),
        Some("surface-only") => (true, false),
        Some(value) => return Err(io::Error::other(format!("unknown scene mode {value}")).into()),
    };

    let options = pdbiox::ReadOptions::new().mode(pdbiox::ParseMode::Recover);
    let (parsed, _diagnostics) = pdbiox::read_with_options(path, &options)
        .map_err(|diagnostics| format!("could not read {path}: {diagnostics:?}"))?;
    let structure = pdbiox::infer_bonds(
        &parsed,
        pdbiox::BondInference::default(),
        &pdbiox::ExecutionContext::default(),
    )
    .map_err(|diagnostics| format!("bond inference failed: {diagnostics:?}"))?
    .structure;
    let (ligand_atoms, ligand_points) = ligand(&structure, ligand_name)?;
    let ligand_bound = BoundingSphere::from_points(&ligand_points);
    let ligand_bonds = ligand_bonds(&structure, &ligand_atoms);
    let ligand_template =
        analytic_ligand_template(ligand_bound.center, &ligand_points, &ligand_bonds)?;

    let mut scene = Scene::new();
    let receptor = scene.add_structure(&structure)?;
    // Archive entries do not always retain the polymer subtype needed by the
    // narrower `protein` predicate after recovery; the receptor polymer is the
    // correct surface source for protein-only docking inputs.
    let protein = scene.select(Select::polymer())?;
    let anchor =
        scene.add_structure_selection(receptor, AtomSelection::Sparse(ligand_atoms.clone()))?;
    let zone = scene.surface_zone_with(
        protein,
        anchor,
        SurfaceZoneStyle {
            distance: 6.0,
            kind: SurfaceKind::SolventExcluded,
            presentation: SurfaceStyle::Solid,
            opacity: 0.42,
            color: Rgba8::opaque(76, 99, 112),
        },
    )?;
    if let Some(surface) = scene.representation_mut(zone.representation) {
        surface.material.roughness = 0.72;
        surface.material.specular = 0.12;
    }
    if !surface_enabled {
        scene.hide(zone.representation);
    }
    let protein_atoms = scene
        .selection_for(protein, receptor)
        .map_or(0, |selection| selection.count(structure.atom_count()));
    let zone_atoms = scene
        .selection_for(zone.selection, receptor)
        .map_or(0, |selection| selection.count(structure.atom_count()));
    add_licorice(&mut scene, anchor, Rgba8::opaque(255, 255, 255))?;

    let template_parts = ligand_template.part_count();
    if instances_enabled {
        let _parts = add_candidates(
            &mut scene,
            ligand_template,
            candidates,
            ligand_bound,
            opacity,
        )?;
    }

    let frame = BoundingSphere {
        center: ligand_bound.center,
        radius: ligand_bound.radius + 8.0,
    };
    let camera = Camera::framing(&frame, 4.0 / 3.0);
    let mut engine = Engine::new(
        &EngineConfig {
            mode,
            ..EngineConfig::default()
        },
        None,
    )?;
    let image = engine.render_image(&scene, &camera, IMAGE)?;
    write_png(output, &image)?;
    println!(
        "registered {candidates} generic instances around {ligand_name}; template parts={template_parts}; transform bytes={}; opacity={opacity:.3}, mode={mode:?}; instances enabled={instances_enabled}; pocket surface enabled={surface_enabled}, uses {zone_atoms}/{protein_atoms} protein atoms; all instances share one analytic template and two homogeneous indirect streams",
        std::mem::size_of::<RigidInstance>(),
    );
    if profile_frames > 0 {
        profile(&mut engine, &scene, &camera, profile_frames)?;
    }
    Ok(())
}

fn add_candidates(
    scene: &mut Scene,
    template: Arc<AnalyticTemplate>,
    count: usize,
    bound: BoundingSphere,
    opacity: f32,
) -> Result<usize, Box<dyn Error>> {
    let row_count = u32::try_from(count)
        .map_err(|_| io::Error::other("candidate count exceeds the u32 row limit"))?;
    if row_count == 0 {
        return Err(io::Error::other("candidate count must be positive").into());
    }
    let mut transforms = Vec::with_capacity(count);
    for index in 0..count {
        transforms.push(candidate_transform(index, bound.center, bound.radius)?);
    }
    let part_count = template.part_count();
    let batch = InstanceBatch::new(
        template,
        Arc::from(transforms),
        SourceRows::ordered(SourceNamespace(0x504f_5345), row_count),
    )?
    .with_style(InstanceStyle {
        color: Rgba8::new(56, 189, 248, opacity_alpha(opacity)),
    });
    let _handle = scene.add_instance_batch(batch);
    Ok(part_count)
}

fn ligand(
    structure: &pdbiox::Structure,
    name: &str,
) -> Result<(Vec<u32>, Vec<Vec3>), Box<dyn Error>> {
    let Some(residue) = structure
        .data()
        .residues()
        .find(|row| row.name() == Some(name))
    else {
        return Err(io::Error::other(format!("component {name} is absent")).into());
    };
    let capacity = residue.atoms().size_hint().0;
    let mut atoms = Vec::with_capacity(capacity);
    let mut points = Vec::with_capacity(capacity);
    for atom in residue.atoms() {
        let row = atom.index().get();
        let Ok(index) = usize::try_from(row) else {
            continue;
        };
        let Some(point) = structure.positions().get(index).copied() else {
            continue;
        };
        atoms.push(row);
        points.push(Vec3::from_array(point));
    }
    if points.is_empty() {
        return Err(io::Error::other("ligand has no positioned atoms").into());
    }
    Ok((atoms, points))
}

fn ligand_bonds(structure: &pdbiox::Structure, rows: &[u32]) -> Vec<[u32; 2]> {
    let mut local = rows
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, row)| u32::try_from(index).ok().map(|index| (row, index)))
        .collect::<Vec<_>>();
    local.sort_unstable_by_key(|&(row, _)| row);
    structure
        .data()
        .bonds
        .iter()
        .filter_map(|bond| {
            let a = local_index(&local, bond.atom_a.get())?;
            let b = local_index(&local, bond.atom_b.get())?;
            Some([a, b])
        })
        .collect()
}

fn local_index(rows: &[(u32, u32)], atom: u32) -> Option<u32> {
    rows.binary_search_by_key(&atom, |&(row, _)| row)
        .ok()
        .map(|index| rows[index].1)
}

fn add_licorice(
    scene: &mut Scene,
    selection: molgfx::SelectionHandle,
    color: Rgba8,
) -> Result<(), Box<dyn Error>> {
    let handle = scene.represent(selection, RepresentationKind::Licorice)?;
    let Some(value) = scene.representation_mut(handle) else {
        return Err(io::Error::other("licorice representation became stale").into());
    };
    value.color = ColorScheme::Uniform(color);
    value.params.radius_scale = 0.30;
    value.params.bond_radius = 0.16;
    value.material.roughness = 0.34;
    value.material.specular = 0.36;
    value.order = 1;
    Ok(())
}

fn analytic_ligand_template(
    center: Vec3,
    points: &[Vec3],
    bonds: &[[u32; 2]],
) -> Result<Arc<AnalyticTemplate>, Box<dyn Error>> {
    let spheres = points
        .iter()
        .map(|point| AnalyticSphere {
            center: (*point - center).to_array(),
            radius: 0.30,
        })
        .collect::<Vec<_>>();
    let mut capsules = Vec::with_capacity(bonds.len());
    for [start, end] in bonds {
        let start = point_at(points, *start)? - center;
        let end = point_at(points, *end)? - center;
        capsules.push(AnalyticCapsule::new(start, end, 0.16)?);
    }
    let part_count = spheres.len().saturating_add(capsules.len());
    let row_count = u32::try_from(part_count)
        .map_err(|_| io::Error::other("ligand template exceeds the u32 row limit"))?;
    Ok(Arc::new(AnalyticTemplate::new(
        Arc::from(spheres),
        Arc::from(capsules),
        SourceRows::ordered(SourceNamespace(0x5445_4d50), row_count),
    )?))
}

fn point_at(points: &[Vec3], row: u32) -> Result<Vec3, io::Error> {
    let index = usize::try_from(row)
        .map_err(|_| io::Error::other("ligand bond index exceeds addressable memory"))?;
    points
        .get(index)
        .copied()
        .ok_or_else(|| io::Error::other("ligand bond index is outside the template"))
}

fn candidate_transform(
    index: usize,
    center: Vec3,
    ligand_radius: f32,
) -> Result<RigidInstance, molgfx::CoreError> {
    let seed = u32::try_from(index)
        .map_or(u32::MAX, |value| value)
        .wrapping_add(1);
    let x = signed_hash(seed.wrapping_mul(0x9e37_79b9));
    let y = signed_hash(seed.wrapping_mul(0x85eb_ca6b));
    let z = signed_hash(seed.wrapping_mul(0xc2b2_ae35));
    let offset = Vec3::new(x, y, z) * ligand_radius.max(1.0) * 1.35;
    let rotation = Quat::from_rotation_y(x * 0.55) * Quat::from_rotation_x(y * 0.55);
    RigidInstance::new(center + offset, rotation, 1.0)
}

fn signed_hash(mut value: u32) -> f32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    let low = u16::try_from(value & 0xffff).map_or(u16::MAX, |bits| bits);
    let unit = f32::from(low) / 65_535.0;
    unit.mul_add(2.0, -1.0)
}

fn opacity_alpha(opacity: f32) -> u8 {
    num_traits::cast((opacity.clamp(0.0, 1.0) * 255.0).round()).map_or(u8::MAX, |alpha| alpha)
}

fn profile(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
    frames: usize,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..3 {
        engine.profile_frame(scene, camera, IMAGE)?;
    }
    let mut gpu = Vec::with_capacity(frames);
    let mut cpu = Vec::with_capacity(frames);
    let mut frame = Vec::with_capacity(frames);
    for _ in 0..frames {
        let timing = engine.profile_frame(scene, camera, IMAGE)?;
        gpu.push(timing.gpu_ns);
        cpu.push(timing.cpu_ns);
        frame.push(timing.frame_ns);
    }
    gpu.sort_unstable();
    cpu.sort_unstable();
    frame.sort_unstable();
    println!(
        "profile: GPU median {} ns; CPU median {} ns; frame median {} ns; frame p99 {} ns ({:.2} FPS)",
        gpu[frames / 2],
        cpu[frames / 2],
        frame[frames / 2],
        p99(&frame),
        fps(p99(&frame))
    );
    let Some(batch_size) = std::num::NonZeroU32::new(8) else {
        return Err(io::Error::other("invalid zero batch size").into());
    };
    let batch_len = usize::try_from(batch_size.get()).map_or(8, |value| value);
    let batches = frames.div_ceil(batch_len).max(1);
    let mut throughput = Vec::with_capacity(batches);
    for _ in 0..batches {
        throughput.push(
            engine
                .profile_frame_batch(scene, camera, IMAGE, batch_size)?
                .frame_ns,
        );
    }
    throughput.sort_unstable();
    println!(
        "throughput batch-8 median {} ns; p99 {} ns ({:.2} FPS)",
        throughput[batches / 2],
        p99(&throughput),
        fps(p99(&throughput))
    );
    Ok(())
}

fn p99(sorted: &[u64]) -> u64 {
    let index = (99 * sorted.len()).div_ceil(100).saturating_sub(1);
    sorted.get(index).copied().map_or(0, |value| value)
}

fn fps(nanoseconds: u64) -> f64 {
    1.0 / std::time::Duration::from_nanos(nanoseconds.max(1)).as_secs_f64()
}

fn parse_usize(
    value: Option<&String>,
    fallback: usize,
    label: &str,
) -> Result<usize, Box<dyn Error>> {
    match value {
        Some(value) => value
            .parse::<usize>()
            .map_err(|error| io::Error::other(format!("invalid {label}: {error}")).into()),
        None => Ok(fallback),
    }
}

fn parse_opacity(value: Option<&String>) -> Result<f32, Box<dyn Error>> {
    let opacity = value.map_or(Ok(1.0), |text| text.parse::<f32>())?;
    if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
        return Err(io::Error::other("opacity must be finite and in [0, 1]").into());
    }
    Ok(opacity)
}

fn write_png(path: impl AsRef<Path>, image: &Image) -> Result<(), Box<dyn Error>> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(file, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
