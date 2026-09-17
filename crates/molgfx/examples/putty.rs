//! Renders crystallographic B factors as a reversible variable-radius tube.

use molgfx::{
    AtomSelection, BackdropStyle, Camera, ColorScheme, EffectLayer, Engine, EngineConfig,
    ImageConfig, Material, PresentationEffect, RenderProfile, RepresentationKind, Rgba8, Scene,
    TubeRadiusMapping,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

const IMAGE: ImageConfig = ImageConfig {
    width: 1_200,
    height: 900,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/4hhb.cif", String::as_str);
    let output = arguments
        .get(1)
        .map_or("target/visual-checks/putty.png", String::as_str);
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let domain = guide_b_factor_domain(&structure)?;
    let mut scene = Scene::from_structure(&structure)?;
    let selection = scene.add_selection(AtomSelection::All);
    let tube = scene.represent(selection, RepresentationKind::Tube)?;
    let representation = scene
        .representation_mut(tube)
        .ok_or_else(|| io::Error::other("putty representation became stale"))?;
    representation.params.tube_radius = 0.32;
    representation.params.tube_radius_mapping = TubeRadiusMapping::b_factor(domain, [0.18, 0.72])?;
    representation.color = ColorScheme::Uniform(Rgba8::opaque(53, 136, 162));
    representation.material = Material {
        roughness: 0.42,
        specular: 0.32,
        ..Material::default()
    };

    let profile = RenderProfile::illustrative().with_layer(EffectLayer::new(
        PresentationEffect::Backdrop(BackdropStyle {
            top: Rgba8::opaque(179, 186, 187),
            bottom: Rgba8::opaque(139, 151, 154),
            glow_color: Rgba8::opaque(205, 210, 207),
            glow_strength: 0.05,
        }),
    ));
    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let mut engine = Engine::new(
        &EngineConfig {
            profile,
            ..EngineConfig::default()
        },
        None,
    )?;
    let image = engine.render_image(&scene, &camera, IMAGE)?;
    write_png(output, &image)?;
    if arguments.get(2).map(String::as_str) == Some("profile") {
        profile_frames(&mut engine, &scene, &camera)?;
    }
    println!(
        "putty B-factor domain [{:.3}, {:.3}] Å²; wrote {output}",
        domain[0], domain[1]
    );
    Ok(())
}

fn guide_b_factor_domain(structure: &pdbiox::Structure) -> Result<[f32; 2], Box<dyn Error>> {
    let mut domain: Option<[f32; 2]> = None;
    for atom in structure
        .data()
        .atoms()
        .filter(|atom| matches!(atom.name(), Some("CA" | "C4'")))
    {
        let Some(value) = atom.b_factor().filter(|value| value.is_finite()) else {
            continue;
        };
        domain = Some(match domain {
            Some([minimum, maximum]) => [minimum.min(value), maximum.max(value)],
            None => [value, value],
        });
    }
    let Some(domain) = domain.filter(|domain| domain[0] < domain[1]) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "putty rendering requires at least two distinct recorded guide-atom B factors",
        )
        .into());
    };
    Ok(domain)
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
        "putty: GPU median {gpu_median} ns ({:.2} FPS), GPU p99 {gpu_p99} ns ({:.2} FPS), CPU median {cpu_median} ns",
        1.0 / std::time::Duration::from_nanos(gpu_median).as_secs_f64(),
        1.0 / std::time::Duration::from_nanos(gpu_p99).as_secs_f64(),
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
