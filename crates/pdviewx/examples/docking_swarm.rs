//! Pocket surface with many deterministic candidate ligand poses in licorice.
//!
//! Usage: `cargo run --release --example docking_swarm --features semantic --
//! [structure] [ligand] [candidate-count] [output.png] [profile-frames]`

use pdviewx::{
    AtomSelection, BoundingSphere, Camera, ColorScheme, Engine, EngineConfig, Image, ImageConfig,
    Mat4, Particle, ParticleShape, Primitive, Quat, RepresentationKind, Rgba8, Scene, Select,
    SurfaceKind, SurfaceStyle, SurfaceZoneScene, SurfaceZoneStyle, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

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

    let options = pdbiox::ReadOptions::new().mode(pdbiox::ParseMode::Recover);
    let (parsed, _diagnostics) = pdbiox::read_with_options(path, &options)
        .map_err(|diagnostics| format!("could not read {path}: {diagnostics:?}"))?;
    let structure = pdbiox::infer_bonds(&parsed, pdbiox::BondInference::default())
        .map_err(|diagnostics| format!("bond inference failed: {diagnostics:?}"))?
        .structure;
    let (ligand_atoms, ligand_points) = ligand(&structure, ligand_name)?;
    let ligand_template = LigandTemplate::new(&structure, &ligand_atoms, ligand_points.clone());
    let ligand_bound = BoundingSphere::from_points(&ligand_points);

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
    let protein_atoms = scene
        .selection_for(protein, receptor)
        .map_or(0, |selection| selection.count(structure.atom_count()));
    let zone_atoms = scene
        .selection_for(zone.selection, receptor)
        .map_or(0, |selection| selection.count(structure.atom_count()));
    add_licorice(&mut scene, anchor, Rgba8::opaque(255, 255, 255))?;

    let mut candidate_primitives = Vec::with_capacity(
        candidates.saturating_mul(ligand_template.atoms.len() + ligand_template.bonds.len()),
    );
    for index in 0..candidates {
        let transform = candidate_transform(index, ligand_bound.center, ligand_bound.radius);
        ligand_template.append_pose(
            receptor,
            transform,
            candidate_color(index),
            &mut candidate_primitives,
        )?;
    }
    scene.add_primitives(&candidate_primitives)?;

    let frame = BoundingSphere {
        center: ligand_bound.center,
        radius: ligand_bound.radius + 8.0,
    };
    let camera = Camera::framing(&frame, 4.0 / 3.0);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    let image = engine.render_image(&scene, &camera, IMAGE)?;
    write_png(output, &image)?;
    println!(
        "rendered {candidates} candidate poses around {ligand_name}; pocket surface uses {zone_atoms}/{protein_atoms} protein atoms; all poses share one native analytic primitive batch"
    );
    if profile_frames > 0 {
        profile(&mut engine, &scene, &camera, profile_frames)?;
    }
    Ok(())
}

struct LigandTemplate {
    atoms: Vec<Vec3>,
    bonds: Vec<(Vec3, Vec3)>,
}

impl LigandTemplate {
    fn new(structure: &pdbiox::Structure, rows: &[u32], atoms: Vec<Vec3>) -> Self {
        let mut local = vec![usize::MAX; structure.positions().len()];
        for (index, row) in rows.iter().copied().enumerate() {
            if let Ok(row) = usize::try_from(row)
                && let Some(slot) = local.get_mut(row)
            {
                *slot = index;
            }
        }
        let bonds = structure
            .data()
            .bonds
            .iter()
            .filter_map(|bond| {
                let a = *local.get(bond.atom_a.as_usize())?;
                let b = *local.get(bond.atom_b.as_usize())?;
                if a == usize::MAX || b == usize::MAX {
                    return None;
                }
                Some((*atoms.get(a)?, *atoms.get(b)?))
            })
            .collect();
        Self { atoms, bonds }
    }

    fn append_pose(
        &self,
        owner: pdviewx::StructureHandle,
        transform: Mat4,
        color: Rgba8,
        out: &mut Vec<Primitive>,
    ) -> Result<(), pdviewx::CoreError> {
        const DIAMETER: f32 = 0.32;
        for center in self.atoms.iter().copied() {
            out.push(Primitive::particle(Particle::new(
                owner,
                transform.transform_point3(center),
                Quat::IDENTITY,
                Vec3::splat(DIAMETER),
                ParticleShape::Sphere,
                color,
                1.0,
            )?));
        }
        for (start, end) in self.bonds.iter().copied() {
            let start = transform.transform_point3(start);
            let end = transform.transform_point3(end);
            let axis = end - start;
            let length = axis.length();
            let Some(direction) = axis.try_normalize() else {
                continue;
            };
            out.push(Primitive::particle(Particle::new(
                owner,
                (start + end) * 0.5,
                Quat::from_rotation_arc(Vec3::Z, direction),
                Vec3::new(DIAMETER, DIAMETER, length + DIAMETER),
                ParticleShape::Spherocylinder,
                color,
                1.0,
            )?));
        }
        Ok(())
    }
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
    let atoms = residue
        .atoms()
        .map(|atom| atom.index().get())
        .collect::<Vec<_>>();
    let points = atoms
        .iter()
        .filter_map(|row| {
            structure
                .positions()
                .get(usize::try_from(*row).ok()?)
                .copied()
                .map(Vec3::from_array)
        })
        .collect::<Vec<_>>();
    if points.is_empty() {
        return Err(io::Error::other("ligand has no positioned atoms").into());
    }
    Ok((atoms, points))
}

fn add_licorice(
    scene: &mut Scene,
    selection: pdviewx::SelectionHandle,
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

fn candidate_transform(index: usize, center: Vec3, ligand_radius: f32) -> Mat4 {
    let seed = u32::try_from(index)
        .map_or(u32::MAX, |value| value)
        .wrapping_add(1);
    let x = signed_hash(seed.wrapping_mul(0x9e37_79b9));
    let y = signed_hash(seed.wrapping_mul(0x85eb_ca6b));
    let z = signed_hash(seed.wrapping_mul(0xc2b2_ae35));
    let offset = Vec3::new(x, y, z) * ligand_radius.max(1.0) * 1.35;
    let rotation = Mat4::from_rotation_y(x * 0.55) * Mat4::from_rotation_x(y * 0.55);
    Mat4::from_translation(center + offset) * rotation * Mat4::from_translation(-center)
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

fn candidate_color(index: usize) -> Rgba8 {
    const COLORS: [Rgba8; 6] = [
        Rgba8::opaque(56, 189, 248),
        Rgba8::opaque(244, 114, 182),
        Rgba8::opaque(74, 222, 128),
        Rgba8::opaque(251, 191, 36),
        Rgba8::opaque(167, 139, 250),
        Rgba8::opaque(248, 113, 113),
    ];
    COLORS[index % COLORS.len()]
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
