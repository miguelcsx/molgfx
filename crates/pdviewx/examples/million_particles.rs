//! Million-item stress over the molecular atom path: columnar source data,
//! compute culling and one indirect analytic-sphere draw.
//!
//! Usage: `cargo run --release --example million_particles --
//! [count] [output.png] [profile-frames] [memory] [spacefill|points]`

use pdbiox::core::topology::{ChainRecord, ResidueRecord};
use pdbiox::core::{OptionalI32, OptionalSymbol};
use pdbiox::{
    AltId, AtomRecord, ChunkBuilder, CoordinateStore, Element, EntityKind, PolymerKind, Presence,
    ResidueIndex, Structure, StructureData,
};
use pdviewx::{
    Camera, ColorScheme, Engine, EngineConfig, Image, ImageConfig, Material, Representation, Rgba8,
    Scene, Select, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

const IMAGE: ImageConfig = ImageConfig {
    width: 1024,
    height: 768,
};
const CHUNK_ATOMS: usize = 4096;

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let count = parse_usize(args.first(), 1_000_000, "atom count")?;
    let count_u32 = u32::try_from(count)
        .map_err(|_| io::Error::other("atom count exceeds the u32 scene limit"))?;
    if count == 0 {
        return Err(io::Error::other("atom count must be positive").into());
    }
    let output = args
        .get(1)
        .map_or("target/visual-checks/million-atoms.png", String::as_str);
    let frames = parse_usize(args.get(2), 12, "profile frames")?;
    let memory = args.get(3).is_some_and(|value| value == "memory");
    let points = args.get(4).is_some_and(|value| value == "points");
    let side = cube_side(count);
    let spacing = 0.18_f32;
    let extent = small_f32(side.saturating_sub(1)) * spacing * 0.5;
    let structure = synthetic_atoms(count, count_u32, side, spacing, extent)?;
    report_memory(memory, "structure");

    let mut scene = Scene::from_structure(&structure)?;
    report_memory(memory, "scene");
    let material = Material {
        roughness: 0.42,
        specular: 0.24,
        ..Material::default()
    };
    let representation = if points {
        Representation::points()
    } else {
        Representation::spacefill()
    };
    scene.represent(
        Select::all(),
        representation
            .color(ColorScheme::Uniform(Rgba8::opaque(52, 156, 219)))
            .radius_scale(spacing * 0.31 / 1.70)
            .material(material),
    )?;

    let bounds = pdviewx::Aabb::new(
        Vec3::splat(-extent - spacing),
        Vec3::splat(extent + spacing),
    );
    let camera = Camera::framing_aabb(&bounds, 4.0 / 3.0);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    report_memory(memory, "engine");
    let first_frame = Instant::now();
    write_png(output, &engine.render_image(&scene, &camera, IMAGE)?)?;
    println!("first frame {} us", first_frame.elapsed().as_micros());
    report_memory(memory, "first-frame");
    let kind = if points { "points" } else { "analytic spheres" };
    println!("rendered {count} {kind} through one indirect representation");
    if frames > 0 {
        profile(&mut engine, &scene, &camera, frames)?;
    }
    Ok(())
}

fn report_memory(enabled: bool, stage: &str) {
    if !enabled {
        return;
    }
    let pid = std::process::id().to_string();
    let rss = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|text| text.trim().parse::<u64>().ok());
    match rss {
        Some(kib) => println!("memory {stage}: {} MiB RSS", kib / 1024),
        None => println!("memory {stage}: unavailable"),
    }
}

fn synthetic_atoms(
    count: usize,
    count_u32: u32,
    side: usize,
    spacing: f32,
    extent: f32,
) -> Result<Structure, Box<dyn Error>> {
    let mut data = StructureData::empty();
    let entity_name = data.dictionary.intern("stress")?;
    let component = data.dictionary.intern("SPH")?;
    let atom_name = data.dictionary.intern("C")?;
    let chain_name = data.dictionary.intern("A")?;
    let residue_count = count.div_ceil(CHUNK_ATOMS);
    let sequence = vec![component; residue_count];
    let entity = data.topology.entities.push(
        entity_name,
        EntityKind::Polymer,
        OptionalSymbol::NONE,
        &sequence,
    )?;
    let mut builder = ChunkBuilder::new();
    builder.reserve(count);
    for residue_index in 0..residue_count {
        let start = residue_index.saturating_mul(CHUNK_ATOMS);
        let end = start.saturating_add(CHUNK_ATOMS).min(count);
        let start_u32 = u32::try_from(start)?;
        let end_u32 = u32::try_from(end)?;
        let residue = data.topology.residues.push(
            ResidueRecord {
                label_comp_id: component,
                auth_comp_id: OptionalSymbol::NONE,
                label_seq_id: OptionalI32::some(i32::try_from(residue_index.saturating_add(1))?),
                auth_seq_id: OptionalI32::NONE,
                ins_code: OptionalSymbol::NONE,
                het: false,
            },
            start_u32..end_u32,
        )?;
        for index in start..end {
            let (x, y, z) = grid(index, side);
            builder.push(AtomRecord {
                position: Some([
                    small_f32(x).mul_add(spacing, -extent),
                    small_f32(y).mul_add(spacing, -extent),
                    small_f32(z).mul_add(spacing, -extent),
                ]),
                element: Element::CARBON,
                atom_name,
                auth_atom_name: OptionalSymbol::NONE,
                alternate_component_id: OptionalSymbol::NONE,
                alt_id: AltId::BLANK,
                residue: ResidueIndex::new(residue.get()),
                occupancy: (1.0, Presence::Present),
                b_factor: (0.0, Presence::Present),
                formal_charge: (0, Presence::Inapplicable),
                atom_site_id: u32::try_from(index)?,
            });
        }
    }
    data.topology.chains.push(
        ChainRecord {
            label_asym_id: chain_name,
            auth_asym_id: OptionalSymbol::some(chain_name),
            entity,
            polymer_kind: PolymerKind::Other,
        },
        0..u32::try_from(residue_count)?,
    )?;
    data.topology.models.push(1, 0..1)?;
    let (chunks, coordinates) = builder.finish();
    data.chunks = chunks.into();
    data.coords = CoordinateStore::Single(coordinates);
    if data.topology.atom_count() != count_u32 {
        return Err(io::Error::other("synthetic topology does not tile all atoms").into());
    }
    Ok(Structure::new(data))
}

fn cube_side(count: usize) -> usize {
    let mut side = 1_usize;
    while side.saturating_mul(side).saturating_mul(side) < count {
        side = side.saturating_add(1);
    }
    side
}

fn grid(index: usize, side: usize) -> (usize, usize, usize) {
    let plane = side.saturating_mul(side);
    (index % side, (index / side) % side, index / plane)
}

fn small_f32(value: usize) -> f32 {
    f32::from(u16::try_from(value).map_or(u16::MAX, |small| small))
}

fn profile(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
    frames: usize,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..2 {
        engine.profile_frame(scene, camera, IMAGE)?;
    }
    let mut gpu = Vec::with_capacity(frames);
    let mut cpu = Vec::with_capacity(frames);
    let mut frame = Vec::with_capacity(frames);
    for _ in 0..frames {
        let timing = engine.profile_frame(scene, camera, IMAGE)?;
        if timing.gpu_ns > 0 {
            gpu.push(timing.gpu_ns);
        }
        cpu.push(timing.cpu_ns);
        frame.push(timing.frame_ns);
    }
    gpu.sort_unstable();
    cpu.sort_unstable();
    frame.sort_unstable();
    println!("CPU median {} ns; p99 {} ns", cpu[frames / 2], p99(&cpu));
    println!(
        "frame median {} ns; p99 {} ns; p99 {:.2} FPS",
        frame[frames / 2],
        p99(&frame),
        fps(p99(&frame))
    );
    match gpu.get(gpu.len() / 2) {
        Some(value) => println!("GPU median {value} ns"),
        None => println!("GPU timing unavailable on this adapter"),
    }
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
        "throughput batch-8 median {} ns; p99 {} ns; p99 {:.2} FPS",
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
