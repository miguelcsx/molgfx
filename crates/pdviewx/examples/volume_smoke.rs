//! Renders a deterministic synthetic scalar-density fixture off screen.
//!
//! Usage: `cargo run --example volume_smoke --release -- output.png [direct|isosurface|medium|slice|crop] [profile] [mrc-path]`

use pdviewx::{
    AtomSelection, BackdropStyle, Camera, ClipPlane, DensityVolume, EffectLayer, Engine,
    EngineConfig, ImageConfig, Material, PresentationEffect, RenderProfile, Representation, Rgba8,
    Scene, Vec3, VolumeRegion, VolumeSlice, VolumeStyle, VolumeTransferFunction,
    VolumeTransferPoint,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

const GRID: u16 = 96;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let output = match arguments.first() {
        Some(output) => output,
        None => "/tmp/pdviewx-volume-smoke.png",
    };
    let algorithm = arguments.get(1).map_or("direct", String::as_str);
    let should_profile = arguments.get(2).is_some_and(|value| value == "profile");
    let volume = arguments
        .get(3)
        .map_or_else(|| synthetic_density(), |path| read_mrc(path))?;
    if let Some(path) = arguments.get(3) {
        println!("volume source: external MRC {path}");
    } else {
        println!("volume source: synthetic");
    }
    let structure = molecular_fixture()?;
    let mut scene = Scene::from_structure(&structure)?;
    let atoms = scene.add_selection(AtomSelection::All);
    scene.represent(
        atoms,
        Representation::ball_and_stick()
            .radius_scale(0.30)
            .bond_radius(0.15)
            .material(Material {
                roughness: 0.48,
                specular: 0.28,
                ..Material::default()
            })
            .order(1),
    )?;
    let volume = scene.add_volume(volume);
    let opacity_scale = match algorithm {
        "medium" => 0.60,
        "slice" => 6.0,
        _ => 1.35,
    };
    let step_scale = if algorithm == "medium" { 0.65 } else { 0.45 };
    let opacities = if algorithm == "medium" {
        [0.0, 0.008, 0.04, 0.18]
    } else {
        [0.0, 0.012, 0.09, 0.48]
    };
    let transfer = VolumeTransferFunction::new(&[
        VolumeTransferPoint::new(0.10, pdviewx::Rgba8::opaque(30, 118, 180), opacities[0]),
        VolumeTransferPoint::new(0.26, pdviewx::Rgba8::opaque(52, 191, 206), opacities[1]),
        VolumeTransferPoint::new(0.62, pdviewx::Rgba8::opaque(160, 218, 166), opacities[2]),
        VolumeTransferPoint::new(1.15, pdviewx::Rgba8::opaque(255, 213, 79), opacities[3]),
    ])?;
    let volume_style = match algorithm {
        "direct" => VolumeStyle::default(),
        "isosurface" => VolumeStyle::isosurface(),
        "medium" => VolumeStyle::medium(),
        "slice" => VolumeStyle::slice(VolumeSlice::new(ClipPlane::from_point_normal(
            Vec3::ZERO,
            Vec3::Z,
        )?)),
        "crop" => VolumeStyle::default().region(VolumeRegion::new(
            [18, 24, 30],
            [78, 72, 66],
            [u32::from(GRID); 3],
        )?),
        name => return Err(format!("unknown volume algorithm {name}").into()),
    };
    scene.represent(
        volume,
        Representation::volume().volume_style(
            volume_style
                .sampling(opacity_scale, step_scale)
                .transfer(transfer),
        ),
    )?;

    let config = ImageConfig {
        width: 960,
        height: 720,
    };
    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let profile = RenderProfile::inspection().with_layer(EffectLayer::new(
        PresentationEffect::Backdrop(BackdropStyle {
            top: Rgba8::opaque(188, 194, 197),
            bottom: Rgba8::opaque(154, 164, 170),
            glow_color: Rgba8::opaque(206, 211, 213),
            glow_strength: 0.04,
        }),
    ));
    let mut engine = Engine::new(
        &EngineConfig {
            profile,
            ..EngineConfig::default()
        },
        None,
    )?;
    println!("pdviewx adapter capabilities: {:?}", engine.capabilities());
    let image = engine.render_image(&scene, &camera, config)?;
    write_png(output, &image)?;
    if should_profile {
        profile_frames(&mut engine, &scene, &camera, config, algorithm)?;
    }
    println!("wrote {output}");
    Ok(())
}

fn profile_frames(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
    config: ImageConfig,
    algorithm: &str,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..20 {
        engine.profile_frame(scene, camera, config)?;
    }
    let mut gpu = Vec::with_capacity(120);
    let mut cpu = Vec::with_capacity(120);
    for _ in 0..120 {
        let timing = engine.profile_frame(scene, camera, config)?;
        gpu.push(timing.gpu_ns);
        cpu.push(timing.cpu_ns);
    }
    gpu.sort_unstable();
    cpu.sort_unstable();
    let percentile = |values: &[u64], numerator: usize| {
        values
            .get((values.len() * numerator).div_ceil(100).saturating_sub(1))
            .copied()
    };
    let (Some(gpu_median), Some(gpu_p99), Some(cpu_median)) = (
        percentile(&gpu, 50),
        percentile(&gpu, 99),
        percentile(&cpu, 50),
    ) else {
        return Err("profiling produced no samples".into());
    };
    println!(
        "{algorithm} volume: GPU median {gpu_median} ns ({:.2} FPS), GPU p99 {gpu_p99} ns ({:.2} FPS), CPU median {cpu_median} ns",
        1.0 / std::time::Duration::from_nanos(gpu_median).as_secs_f64(),
        1.0 / std::time::Duration::from_nanos(gpu_p99).as_secs_f64(),
    );
    Ok(())
}

fn synthetic_density() -> Result<DensityVolume, Box<dyn Error>> {
    let mut values = Vec::with_capacity(usize::from(GRID).pow(3));
    let center = (f32::from(GRID) - 1.0) * 0.5;
    for z in 0..GRID {
        for y in 0..GRID {
            for x in 0..GRID {
                let point = Vec3::new(
                    f32::from(x) - center,
                    f32::from(y) - center,
                    f32::from(z) - center,
                ) / center;
                let folded = gaussian(
                    point,
                    Vec3::new(-0.30, 0.02, 0.08),
                    Vec3::new(0.46, 0.31, 0.38),
                ) + gaussian(
                    point,
                    Vec3::new(0.34, 0.10, -0.08),
                    Vec3::new(0.36, 0.43, 0.30),
                ) * 0.88
                    + gaussian(point, Vec3::new(0.02, -0.40, 0.18), Vec3::splat(0.22)) * 0.52;
                let pocket = gaussian(
                    point,
                    Vec3::new(0.02, 0.02, 0.20),
                    Vec3::new(0.17, 0.22, 0.18),
                );
                let fine_detail = gaussian(
                    point,
                    Vec3::new(-0.08, 0.22, -0.29),
                    Vec3::new(0.13, 0.10, 0.16),
                ) * 0.24;
                values.push((folded + fine_detail - pocket * 0.46).max(0.0));
            }
        }
    }
    let extent = [u32::from(GRID); 3];
    let spacing = Vec3::splat(0.08);
    let origin = Vec3::splat(-center * spacing.x);
    Ok(DensityVolume::from_spacing(
        extent,
        origin,
        spacing,
        Arc::from(values),
    )?)
}

fn read_mrc(path: &str) -> Result<DensityVolume, Box<dyn Error>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "MRC header is truncated").into());
    }
    let dimensions = [
        read_u32(&bytes, 0)?,
        read_u32(&bytes, 4)?,
        read_u32(&bytes, 8)?,
    ];
    if dimensions.iter().any(|&value| value < 2) {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "MRC dimensions are invalid").into(),
        );
    }
    if read_u32(&bytes, 12)? != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the comparison reader supports little-endian MRC MODE 2 only",
        )
        .into());
    }
    let axis_order = [
        read_u32(&bytes, 64)?,
        read_u32(&bytes, 68)?,
        read_u32(&bytes, 72)?,
    ];
    if axis_order != [1, 2, 3] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MRC axis order must be X, Y, Z for this bounded probe",
        )
        .into());
    }
    let sampling = [
        read_u32(&bytes, 28)?,
        read_u32(&bytes, 32)?,
        read_u32(&bytes, 36)?,
    ];
    if sampling.contains(&0) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "MRC sampling is invalid").into());
    }
    let cell_lengths = [
        read_f32(&bytes, 40)?,
        read_f32(&bytes, 44)?,
        read_f32(&bytes, 48)?,
    ];
    let spacing = Vec3::new(
        cell_lengths[0] / dimension_f32(sampling[0]),
        cell_lengths[1] / dimension_f32(sampling[1]),
        cell_lengths[2] / dimension_f32(sampling[2]),
    );
    let origin = Vec3::new(
        read_f32(&bytes, 196)?,
        read_f32(&bytes, 200)?,
        read_f32(&bytes, 204)?,
    );
    let voxel_count = dimensions.iter().try_fold(1usize, |product, &value| {
        product.checked_mul(usize::try_from(value).ok()?)
    });
    let Some(voxel_count) = voxel_count else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "MRC voxel count overflows").into());
    };
    let data_offset = 1024usize
        .checked_add(usize::try_from(read_u32(&bytes, 92)?).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "MRC symmetry-byte count overflows",
            )
        })?)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "MRC data offset overflows"))?;
    let data_bytes = voxel_count
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "MRC data size overflows"))?;
    let data = bytes
        .get(
            data_offset..data_offset.checked_add(data_bytes).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "MRC data end overflows")
            })?,
        )
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "MRC data is truncated"))?;
    let values = data
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|chunk| {
            let raw: [u8; 4] = chunk.try_into().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "MRC value is truncated")
            })?;
            Ok(f32::from_le_bytes(raw))
        })
        .collect::<Result<Vec<_>, io::Error>>()?;
    Ok(DensityVolume::from_spacing(
        dimensions,
        origin,
        spacing,
        Arc::from(values),
    )?)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, io::Error> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "MRC header is truncated"))?
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "MRC header field is truncated"))?;
    Ok(u32::from_le_bytes(raw))
}

fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, io::Error> {
    Ok(f32::from_bits(read_u32(bytes, offset)?))
}

fn dimension_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).map_or(u16::MAX, |converted| converted))
}

fn molecular_fixture() -> Result<pdbiox::Structure, Box<dyn Error>> {
    let source = "data_medium\n\
loop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n\
_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n\
_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n\
HETATM 1 C C1 LIG A 1 -2.2 0.0 0.0\n\
HETATM 2 C C2 LIG A 1 -1.1 0.8 0.0\n\
HETATM 3 N N1 LIG A 1 0.0 0.3 0.2\n\
HETATM 4 C C3 LIG A 1 1.1 1.0 0.0\n\
HETATM 5 O O1 LIG A 1 2.2 0.5 -0.1\n\
HETATM 6 C C4 LIG A 1 0.1 -1.0 -0.2\n\
HETATM 7 S S1 LIG A 1 1.4 -1.5 0.1\n\
HETATM 8 O O2 LIG A 1 -1.2 -1.2 0.3\n";
    let (parsed, _) = pdbiox::read_bytes(
        source.as_bytes().to_vec(),
        Some("medium-fixture.cif"),
        &pdbiox::ReadOptions::default(),
    )
    .map_err(|diagnostics| format!("medium fixture diagnostics: {diagnostics:?}"))?;
    Ok(
        pdbiox::infer_bonds(&parsed, pdbiox::BondInference::default())
            .map_err(|diagnostic| format!("medium fixture bonds: {diagnostic:?}"))?
            .structure,
    )
}

fn gaussian(point: Vec3, center: Vec3, sigma: Vec3) -> f32 {
    let normalized = (point - center) / sigma;
    (-0.5 * normalized.length_squared()).exp()
}

fn write_png(path: impl AsRef<Path>, image: &pdviewx::Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
