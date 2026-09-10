//! Renders a caller-supplied atom property over a biological context.
//!
//! Recorded crystallographic waters remain visible as sparse scene matter; the
//! backdrop is only a compositing fallback and is not presented as environment.

use pdviewx::{
    AttributeColumn, AttributeHandle, AttributeValues, BackdropStyle, Camera, Engine, EngineConfig,
    Image, ImageConfig, Material, PresentationEffect, RenderProfile, RepresentationKind, RowDomain,
    ScalarRamp, Scene, Select, VisualProgramBuilder, VisualStyle,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

const IMAGE: ImageConfig = ImageConfig {
    width: 1_200,
    height: 900,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/4hhb.cif", String::as_str);
    let output = arguments.get(1).map_or(
        "target/visual-checks/property-landscape.png",
        String::as_str,
    );
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let values = structure
        .data()
        .atoms()
        .map(|atom| match atom.b_factor() {
            Some(value) => value,
            None => f32::NAN,
        })
        .collect::<Vec<_>>();
    let mut scene = Scene::from_structure(&structure)?;
    let owner = scene
        .structures()
        .next()
        .map(|(handle, _)| handle)
        .ok_or_else(|| io::Error::other("structure placement is absent"))?;
    let domain = display_domain(&values)?;
    let attribute = scene.add_attribute(AttributeColumn::new(
        RowDomain::Atoms(owner),
        "recorded B factor",
        AttributeValues::Scalar(Arc::from(values)),
    )?)?;
    add_biological_layers(&mut scene, attribute, domain)?;

    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let transparent = arguments.iter().any(|value| value == "transparent");
    let profile = if transparent {
        RenderProfile::inspection()
            .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent()))
    } else {
        RenderProfile::inspection()
    };
    let mut engine = Engine::new(
        &EngineConfig {
            profile,
            ..EngineConfig::default()
        },
        None,
    )?;
    let image = engine.render_image(&scene, &camera, IMAGE)?;
    let coverage = alpha_domain(&image);
    write_png(output, &image)?;
    if arguments.iter().any(|value| value == "profile") {
        profile_frames(&mut engine, &scene, &camera)?;
    }
    let [low, high] = domain;
    println!(
        "recorded B-factor domain [{low:.2}, {high:.2}] A^2; alpha [{}, {}]; wrote {output}",
        coverage[0], coverage[1]
    );
    Ok(())
}

fn add_biological_layers(
    scene: &mut Scene,
    attribute: AttributeHandle,
    domain: [f32; 2],
) -> Result<(), Box<dyn Error>> {
    let protein = scene.select(Select::polymer())?;
    let protein_view = scene.represent(protein, RepresentationKind::Cartoon)?;
    let mut builder = VisualProgramBuilder::new();
    let value = builder.scalar_attribute(attribute)?;
    let color = builder.ramp(value, ScalarRamp::sequential(domain))?;
    let low = builder.scalar(domain[0])?;
    let high = builder.scalar(domain[1])?;
    let weight = builder.smoothstep(low, high, value)?;
    let faint = builder.scalar(0.24)?;
    let opaque = builder.scalar(1.0)?;
    let opacity = builder.mix_scalar(faint, opaque, weight)?;
    builder.set_base_color(color)?;
    builder.set_opacity(opacity)?;
    let visual = VisualStyle::new(builder.finish()?);
    let representation = scene
        .representation_mut(protein_view)
        .ok_or_else(|| io::Error::other("protein representation became stale"))?;
    representation.visual = Some(visual);
    representation.params.ribbon_width = 1.35;
    representation.material = Material::anisotropic_ribbon(0.42);
    representation.material.roughness = 0.48;
    representation.material.specular = 0.28;

    let ligands = scene.select(Select::ligands())?;
    let ligand_view = scene.represent(ligands, RepresentationKind::BallAndStick)?;
    let representation = scene
        .representation_mut(ligand_view)
        .ok_or_else(|| io::Error::other("ligand representation became stale"))?;
    representation.params.radius_scale = 0.34;
    representation.params.bond_radius = 0.18;
    representation.material.roughness = 0.42;
    representation.material.specular = 0.30;
    representation.order = 2;

    let water = scene.select(Select::water())?;
    let water_view = scene.represent(water, RepresentationKind::Spacefill)?;
    let representation = scene
        .representation_mut(water_view)
        .ok_or_else(|| io::Error::other("water representation became stale"))?;
    representation.params.radius_scale = 0.18;
    representation.material.opacity = 0.34;
    representation.material.roughness = 0.30;
    representation.material.specular = 0.32;
    representation.order = 1;
    Ok(())
}

fn display_domain(values: &[f32]) -> Result<[f32; 2], io::Error> {
    let mut finite = values.iter().copied().filter(|value| value.is_finite());
    let Some(first) = finite.next() else {
        return Err(io::Error::other("property has no finite values"));
    };
    let [low, high] = finite.fold([first, first], |[low, high], value| {
        [low.min(value), high.max(value)]
    });
    Ok(if low < high {
        [low, high]
    } else {
        [low - 0.5, high + 0.5]
    })
}

fn profile_frames(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..20 {
        engine.profile_frame(scene, camera, IMAGE)?;
    }
    let mut gpu = Vec::with_capacity(120);
    let mut cpu = Vec::with_capacity(120);
    for _ in 0..120 {
        let timing = engine.profile_frame(scene, camera, IMAGE)?;
        gpu.push(timing.gpu_ns);
        cpu.push(timing.cpu_ns);
    }
    gpu.sort_unstable();
    cpu.sort_unstable();
    let percentile = |values: &[u64], percentile: usize| {
        values
            .get((values.len() * percentile).div_ceil(100).saturating_sub(1))
            .copied()
    };
    let (Some(gpu_median), Some(gpu_p99), Some(cpu_median)) = (
        percentile(&gpu, 50),
        percentile(&gpu, 99),
        percentile(&cpu, 50),
    ) else {
        return Err(io::Error::other("profiling produced no samples").into());
    };
    println!(
        "property landscape: GPU median {gpu_median} ns ({:.2} FPS), GPU p99 {gpu_p99} ns ({:.2} FPS), CPU median {cpu_median} ns",
        fps(gpu_median),
        fps(gpu_p99),
    );
    Ok(())
}

fn fps(nanoseconds: u64) -> f64 {
    1.0 / std::time::Duration::from_nanos(nanoseconds.max(1)).as_secs_f64()
}

fn alpha_domain(image: &Image) -> [u8; 2] {
    image
        .pixels
        .chunks_exact(4)
        .map(|pixel| pixel[3])
        .fold([u8::MAX, u8::MIN], |[low, high], alpha| {
            [low.min(alpha), high.max(alpha)]
        })
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
