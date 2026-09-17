//! Renders a caller-supplied calibrated scalar field on a molecular surface.

use molgfx::{
    AtomSelection, Camera, ColorScheme, Engine, EngineConfig, ImageConfig, RepresentationKind,
    Rgba8, ScalarContours, ScalarFieldSemantics, ScalarRamp, ScalarVolume, Scene, SurfaceKind,
    SurfaceScalarOverlay, Vec3,
};
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

const GRID: u16 = 64;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let structure_path = arguments
        .first()
        .map_or("benchmarks/scenes/optics-depth-stack.cif", String::as_str);
    let output = arguments
        .get(1)
        .map_or("target/visual-checks/scalar-overlay.png", String::as_str);
    let structure = pdbiox::read(structure_path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let mut scene = Scene::from_structure(&structure)?;
    let field = calibrated_fixture(&scene)?;
    let field = scene.add_volume(field);
    let selection = scene.add_selection(AtomSelection::All);
    let surface = scene.represent(selection, RepresentationKind::Surface)?;
    let overlay = SurfaceScalarOverlay {
        contours: Some(ScalarContours::new(0.25, 1.15)?),
        ..SurfaceScalarOverlay::new(field, ScalarRamp::diverging(1.0))
    };
    let Some(representation) = scene.representation_mut(surface) else {
        return Err(io::Error::other("surface became stale").into());
    };
    representation.params.surface_kind = SurfaceKind::VanDerWaals;
    representation.color = ColorScheme::Uniform(Rgba8::WHITE);
    representation.surface_scalar = Some(overlay);

    let config = ImageConfig {
        width: 960,
        height: 720,
    };
    let camera = Camera::framing_aabb(&scene.world_aabb(), 4.0 / 3.0);
    let mut engine = Engine::new(&EngineConfig::default(), None)?;
    let image = engine.render_image(&scene, &camera, config)?;
    write_png(output, &image)?;
    if arguments.get(2).map(String::as_str) == Some("profile") {
        let Some(representation) = scene.representation_mut(surface) else {
            return Err(io::Error::other("surface became stale").into());
        };
        representation.surface_scalar = None;
        profile(&mut engine, &scene, &camera, config, "surface baseline")?;
        let Some(representation) = scene.representation_mut(surface) else {
            return Err(io::Error::other("surface became stale").into());
        };
        representation.surface_scalar = Some(overlay);
        profile(&mut engine, &scene, &camera, config, "scalar overlay")?;
    }
    let volume = scene
        .volume(field)
        .ok_or_else(|| io::Error::other("field became stale"))?;
    println!("scalar field: {:?}", volume.semantics());
    println!("wrote {output}");
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
    let p99_index = (samples.len() * 99).div_ceil(100).saturating_sub(1);
    let p99 = samples
        .get(p99_index)
        .copied()
        .ok_or_else(|| io::Error::other("profiling percentile is absent"))?;
    let median_f64 = f64::from(u32::try_from(median).map_or(u32::MAX, |value| value));
    let p99_f64 = f64::from(u32::try_from(p99).map_or(u32::MAX, |value| value));
    println!(
        "{label}: GPU median {median} ns ({:.2} FPS), p99 {p99} ns ({:.2} FPS)",
        1_000_000_000.0 / median_f64.max(1.0),
        1_000_000_000.0 / p99_f64.max(1.0),
    );
    Ok(())
}

fn calibrated_fixture(scene: &Scene) -> Result<ScalarVolume, Box<dyn Error>> {
    let bounds = scene.world_aabb();
    let padding = Vec3::splat(3.0);
    let origin = bounds.min - padding;
    let extent = bounds.max - bounds.min + padding * 2.0;
    let denominator = f32::from(GRID - 1);
    let spacing = extent / denominator;
    let mut values = Vec::with_capacity(usize::from(GRID).pow(3));
    for z in 0..GRID {
        for y in 0..GRID {
            for x in 0..GRID {
                let normalized = Vec3::new(f32::from(x), f32::from(y), f32::from(z)) / denominator;
                let signed_x = normalized.x * 2.0 - 1.0;
                let modulation = (normalized.y * std::f32::consts::TAU).sin() * 0.22;
                values.push((signed_x + modulation).clamp(-1.0, 1.0));
            }
        }
    }
    let semantics = ScalarFieldSemantics::quantity(
        Arc::from("synthetic electrostatic potential"),
        Arc::from("kT/e"),
        Arc::from("molgfx scalar-overlay visual fixture"),
    )?;
    Ok(
        ScalarVolume::from_spacing([u32::from(GRID); 3], origin, spacing, Arc::from(values))?
            .with_semantics(semantics),
    )
}

fn write_png(path: impl AsRef<Path>, image: &molgfx::Image) -> Result<(), Box<dyn Error>> {
    let mut encoder = png::Encoder::new(File::create(path)?, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}
