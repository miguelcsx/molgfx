//! Compares the neutral inspection default with explicit principled presentation.

use pdviewx::{
    AtomSelection, Camera, ColorScheme, Engine, EngineConfig, Image, ImageConfig, Mat4, Material,
    RepresentationHandle, RepresentationKind, Rgba8, Scene, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;

const IMAGE: ImageConfig = ImageConfig {
    width: 1_200,
    height: 600,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/optics-depth-stack.cif", String::as_str);
    let output = arguments
        .get(1)
        .map_or("target/visual-checks/materials.png", String::as_str);
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let (mut scene, representations) = comparison_scene(&structure)?;
    let camera = Camera::framing_aabb(&scene.world_aabb(), 2.0);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    write_png(output, &engine.render_image(&scene, &camera, IMAGE)?)?;

    if arguments.get(2).map(String::as_str) == Some("profile") {
        profile_materials(&mut engine, &mut scene, representations, &camera)?;
    }
    println!("wrote {output}");
    Ok(())
}

fn comparison_scene(
    structure: &pdbiox::Structure,
) -> Result<(Scene, [RepresentationHandle; 3]), Box<dyn Error>> {
    let mut scene = Scene::new();
    let first = scene.add_structure(structure)?;
    let width = scene
        .structure(first)
        .map(|placed| placed.world_aabb().half_extents().x * 2.0)
        .ok_or_else(|| io::Error::other("structure placement became stale"))?;
    let spacing = width + 6.0;
    let structures = [
        first,
        scene.add_structure(structure)?,
        scene.add_structure(structure)?,
    ];
    let offsets = [-spacing, 0.0, spacing];
    for (handle, offset) in structures.into_iter().zip(offsets) {
        let placed = scene
            .structure_mut(handle)
            .ok_or_else(|| io::Error::other("structure placement became stale"))?;
        placed.model_to_world = Mat4::from_translation(Vec3::new(offset, 0.0, 0.0));
    }

    let colors = [
        Rgba8::opaque(50, 142, 150),
        Rgba8::opaque(74, 118, 202),
        Rgba8::opaque(205, 146, 48),
    ];
    let mut representations = Vec::with_capacity(3);
    for (handle, color) in structures.into_iter().zip(colors) {
        let selection = scene.add_structure_selection(handle, AtomSelection::All)?;
        let representation = scene.represent(selection, RepresentationKind::Spacefill)?;
        let value = scene
            .representation_mut(representation)
            .ok_or_else(|| io::Error::other("representation became stale"))?;
        value.color = ColorScheme::Uniform(color);
        representations.push(representation);
    }
    let representations: [RepresentationHandle; 3] = representations
        .try_into()
        .map_err(|_| io::Error::other("material comparison requires three representations"))?;
    set_materials(&mut scene, representations, MaterialMode::Comparison)?;
    Ok((scene, representations))
}

#[derive(Clone, Copy)]
enum MaterialMode {
    Molecular,
    Principled,
    Comparison,
}

fn set_materials(
    scene: &mut Scene,
    handles: [RepresentationHandle; 3],
    mode: MaterialMode,
) -> Result<(), Box<dyn Error>> {
    for (index, handle) in handles.into_iter().enumerate() {
        let material = match mode {
            MaterialMode::Molecular => Material::default(),
            MaterialMode::Comparison if index == 0 => Material::default(),
            MaterialMode::Principled | MaterialMode::Comparison => polished(index == 2),
        };
        let representation = scene
            .representation_mut(handle)
            .ok_or_else(|| io::Error::other("representation became stale"))?;
        representation.material = material;
    }
    Ok(())
}

fn polished(metallic: bool) -> Material {
    Material {
        roughness: 0.24,
        ..Material::principled(if metallic { 1.0 } else { 0.0 })
    }
}

fn profile_materials(
    engine: &mut Engine,
    scene: &mut Scene,
    handles: [RepresentationHandle; 3],
    camera: &Camera,
) -> Result<(), Box<dyn Error>> {
    for index in 0..24 {
        set_materials(scene, handles, alternating_mode(index))?;
        engine.profile_frame(scene, camera, IMAGE)?;
    }
    let mut molecular = TimingSamples::with_capacity(40);
    let mut principled = TimingSamples::with_capacity(40);
    for index in 0..80 {
        let mode = alternating_mode(index);
        set_materials(scene, handles, mode)?;
        let timing = engine.profile_frame(scene, camera, IMAGE)?;
        match mode {
            MaterialMode::Molecular => molecular.push(&timing),
            MaterialMode::Principled => principled.push(&timing),
            MaterialMode::Comparison => {}
        }
    }
    print_samples("molecular material", &mut molecular)?;
    print_samples("principled material", &mut principled)
}

fn alternating_mode(index: usize) -> MaterialMode {
    if index.is_multiple_of(2) {
        MaterialMode::Molecular
    } else {
        MaterialMode::Principled
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

    fn push(&mut self, timing: &pdviewx::FrameTiming) {
        self.gpu.push(timing.gpu_ns);
        self.cpu.push(timing.cpu_ns);
    }
}

fn print_samples(label: &str, samples: &mut TimingSamples) -> Result<(), Box<dyn Error>> {
    samples.gpu.sort_unstable();
    samples.cpu.sort_unstable();
    let median = |samples: &[u64]| {
        samples
            .get(samples.len() / 2)
            .copied()
            .ok_or_else(|| io::Error::other("profiling produced no samples"))
    };
    let p99 = |samples: &[u64]| {
        samples
            .get((samples.len() * 99).div_ceil(100).saturating_sub(1))
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
