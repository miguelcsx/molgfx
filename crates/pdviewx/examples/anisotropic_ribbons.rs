//! Compares isotropic and tangent-aligned lighting on the same caller ribbon.

use pdviewx::{
    AtomSelection, Camera, ColorScheme, Engine, EngineConfig, FrameTiming, Image, ImageConfig,
    Mat4, Material, RepresentationHandle, RepresentationKind, Rgba8, Scene, SecondaryStructure,
    StructureHandle, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

const IMAGE: ImageConfig = ImageConfig {
    width: 1_400,
    height: 700,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/4hhb.cif", String::as_str);
    let output = arguments.get(1).map_or(
        "target/visual-checks/anisotropic-ribbons.png",
        String::as_str,
    );
    let transparent_output = arguments.get(3).map_or(
        "target/visual-checks/anisotropic-ribbons-transparent.png",
        String::as_str,
    );
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let (mut scene, ribbons) = comparison_scene(&structure)?;
    let camera = Camera::framing_aabb(&scene.world_aabb(), 2.0);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    write_png(output, &engine.render_image(&scene, &camera, IMAGE)?)?;
    set_opacity(&mut scene, ribbons, 0.58)?;
    write_png(
        transparent_output,
        &engine.render_image(&scene, &camera, IMAGE)?,
    )?;
    if arguments.get(2).map(String::as_str) == Some("profile") {
        profile_materials(&mut engine, &mut scene, ribbons, &camera)?;
    }
    println!("wrote {output}");
    println!("wrote {transparent_output}");
    Ok(())
}

fn comparison_scene(
    structure: &pdbiox::Structure,
) -> Result<(Scene, [RepresentationHandle; 2]), Box<dyn Error>> {
    let mut scene = Scene::new();
    let first = scene.add_structure(structure)?;
    let width = scene
        .structure(first)
        .map(|placed| placed.world_aabb().half_extents().x * 2.0)
        .ok_or_else(|| io::Error::other("structure placement became stale"))?;
    let second = scene.add_structure(structure)?;
    let structures = [first, second];
    for (handle, offset) in structures
        .into_iter()
        .zip([-(width + 8.0) * 0.5, (width + 8.0) * 0.5])
    {
        let placed = scene
            .structure_mut(handle)
            .ok_or_else(|| io::Error::other("structure placement became stale"))?;
        placed.model_to_world = Mat4::from_translation(Vec3::new(offset, 0.0, 0.0));
        apply_fixture_secondary_structure(&mut scene, handle)?;
    }

    let mut ribbons = Vec::with_capacity(2);
    for handle in structures {
        let selection = scene.add_structure_selection(handle, AtomSelection::All)?;
        let ribbon = scene.represent(selection, RepresentationKind::Cartoon)?;
        let representation = scene
            .representation_mut(ribbon)
            .ok_or_else(|| io::Error::other("ribbon became stale"))?;
        representation.color = ColorScheme::Uniform(Rgba8::opaque(78, 150, 190));
        representation.params.ribbon_width = 1.8;
        representation.material.roughness = 0.38;
        ribbons.push(ribbon);
    }
    let ribbons: [RepresentationHandle; 2] = ribbons
        .try_into()
        .map_err(|_| io::Error::other("comparison requires two ribbons"))?;
    set_materials(&mut scene, ribbons, MaterialMode::Comparison)?;
    Ok((scene, ribbons))
}

fn apply_fixture_secondary_structure(
    scene: &mut Scene,
    handle: StructureHandle,
) -> Result<(), Box<dyn Error>> {
    let count = scene
        .structure(handle)
        .map(|placed| placed.hierarchy.residue_count())
        .ok_or_else(|| io::Error::other("structure placement became stale"))?;
    let records = (0..count)
        .filter_map(|index| u32::try_from(index).ok())
        .map(|index| (pdbiox::ResidueIndex::new(index), SecondaryStructure::Helix))
        .collect::<Vec<_>>();
    scene.apply_secondary_structure(handle, &records)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum MaterialMode {
    Molecular,
    Anisotropic,
    Comparison,
}

fn set_materials(
    scene: &mut Scene,
    ribbons: [RepresentationHandle; 2],
    mode: MaterialMode,
) -> Result<(), Box<dyn Error>> {
    for (index, handle) in ribbons.into_iter().enumerate() {
        let material = match mode {
            MaterialMode::Molecular => molecular_ribbon(),
            MaterialMode::Comparison if index == 0 => molecular_ribbon(),
            MaterialMode::Anisotropic | MaterialMode::Comparison => anisotropic_ribbon(),
        };
        let representation = scene
            .representation_mut(handle)
            .ok_or_else(|| io::Error::other("ribbon became stale"))?;
        representation.material = material;
    }
    Ok(())
}

fn molecular_ribbon() -> Material {
    Material {
        roughness: 0.38,
        ..Material::default()
    }
}

fn anisotropic_ribbon() -> Material {
    Material {
        roughness: 0.38,
        ..Material::anisotropic_ribbon(0.82)
    }
}

fn set_opacity(
    scene: &mut Scene,
    ribbons: [RepresentationHandle; 2],
    opacity: f32,
) -> Result<(), Box<dyn Error>> {
    for handle in ribbons {
        let representation = scene
            .representation_mut(handle)
            .ok_or_else(|| io::Error::other("ribbon became stale"))?;
        representation.material.opacity = opacity;
    }
    Ok(())
}

fn profile_materials(
    engine: &mut Engine,
    scene: &mut Scene,
    ribbons: [RepresentationHandle; 2],
    camera: &Camera,
) -> Result<(), Box<dyn Error>> {
    for index in 0..24 {
        set_materials(scene, ribbons, alternating_mode(index))?;
        engine.profile_frame(scene, camera, IMAGE)?;
    }
    let mut molecular = TimingSamples::with_capacity(40);
    let mut anisotropic = TimingSamples::with_capacity(40);
    for index in 0..80 {
        let mode = alternating_mode(index);
        set_materials(scene, ribbons, mode)?;
        let timing = engine.profile_frame(scene, camera, IMAGE)?;
        match mode {
            MaterialMode::Molecular => molecular.push(&timing),
            MaterialMode::Anisotropic => anisotropic.push(&timing),
            MaterialMode::Comparison => {}
        }
    }
    print_samples("molecular ribbon", &mut molecular)?;
    print_samples("anisotropic ribbon", &mut anisotropic)
}

fn alternating_mode(index: usize) -> MaterialMode {
    if index.is_multiple_of(2) {
        MaterialMode::Molecular
    } else {
        MaterialMode::Anisotropic
    }
}

struct TimingSamples {
    gpu: Vec<u64>,
    cpu: Vec<u64>,
}

impl TimingSamples {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            gpu: Vec::with_capacity(capacity),
            cpu: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, timing: &FrameTiming) {
        self.gpu.push(timing.gpu_ns);
        self.cpu.push(timing.cpu_ns);
    }
}

fn print_samples(label: &str, samples: &mut TimingSamples) -> Result<(), Box<dyn Error>> {
    samples.gpu.sort_unstable();
    samples.cpu.sort_unstable();
    let median = |values: &[u64]| {
        values
            .get(values.len() / 2)
            .copied()
            .ok_or_else(|| io::Error::other("profiling produced no samples"))
    };
    let p99 = |values: &[u64]| {
        values
            .get((values.len() * 99).div_ceil(100).saturating_sub(1))
            .copied()
            .ok_or_else(|| io::Error::other("profiling produced no samples"))
    };
    let gpu_median = median(&samples.gpu)?;
    let gpu_p99 = p99(&samples.gpu)?;
    let cpu_median = median(&samples.cpu)?;
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
