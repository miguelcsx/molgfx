//! Whole-graph benchmark for semantic ligand focus and context composition.

use pdviewx::{
    AtomSelection, BoundingSphere, Camera, Engine, EngineConfig, FocusScene, ImageConfig,
    RenderProfile, RepresentationKind, Scene, Vec3,
};
use pdviewx_bench::{FrameSample, summarize};
use std::error::Error;
use std::io;
use std::time::Instant;

const WARMUP_FRAMES: usize = 20;
const MEASURED_FRAMES: usize = 120;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let path = required(&arguments, 0, "a structure path")?;
    let component = required(&arguments, 1, "a component name")?;
    let parsed = pdbiox::read(path).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("structure diagnostics: {diagnostics:?}"),
        )
    })?;
    let inference_start = Instant::now();
    let structure = pdbiox::infer_bonds(&parsed, pdbiox::BondInference::default())
        .map_err(|diagnostic| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bond inference diagnostic: {diagnostic:?}"),
            )
        })?
        .structure;
    let inference_ns = inference_start.elapsed().as_nanos();
    let (indices, points) = component_atoms(&structure, component)?;

    let scene_start = Instant::now();
    let mut scene = Scene::from_structure(&structure)?;
    let scene_build_ns = scene_start.elapsed().as_nanos();
    let focus = scene.add_selection(AtomSelection::Sparse(indices));
    let focus_start = Instant::now();
    let view = scene.focus(focus)?;
    let Some(neighbourhood) = scene.representation_mut(view.near_representation) else {
        return Err(io::Error::other("neighbourhood handle became stale").into());
    };
    neighbourhood.kind = RepresentationKind::Lines;
    neighbourhood.params.line_width_pixels = 1.10;
    neighbourhood.material.opacity = 0.44;
    let focus_build_ns = focus_start.elapsed().as_nanos();

    let bound = BoundingSphere::from_points(&points);
    let focus_bound = BoundingSphere {
        center: bound.center,
        radius: bound.radius * 1.72,
    };
    let mut camera = Camera::framing(&focus_bound, 4.0 / 3.0);
    camera
        .projection
        .fit_near_far(camera.eye, &scene.world_aabb().bounding_sphere());
    let config = ImageConfig {
        width: 1280,
        height: 960,
    };
    let mut engine = Engine::new(
        &EngineConfig {
            profile: RenderProfile::cinematic(),
            ..EngineConfig::default()
        },
        None,
    )?;
    for _ in 0..WARMUP_FRAMES {
        engine.profile_frame(&scene, &camera, config)?;
    }
    let mut samples = Vec::with_capacity(MEASURED_FRAMES);
    for _ in 0..MEASURED_FRAMES {
        let timing = engine.profile_frame(&scene, &camera, config)?;
        samples.push(FrameSample {
            gpu_ns: timing.gpu_ns,
            cpu_ns: timing.cpu_ns,
            ..FrameSample::default()
        });
    }
    let summary = summarize(&samples, &mut Vec::with_capacity(samples.len()))?;
    println!("component={component}");
    println!("atoms={}", structure.atom_count());
    println!("bond_inference_ns={inference_ns}");
    println!("scene_build_ns={scene_build_ns}");
    println!("focus_build_ns={focus_build_ns}");
    println!("frames={MEASURED_FRAMES}");
    println!("gpu_median_ns={}", summary.gpu_median_ns);
    println!("gpu_p99_ns={}", summary.gpu_p99_ns);
    println!("cpu_median_ns={}", summary.cpu_median_ns);
    println!("gpu_median_fps={:.2}", fps(summary.gpu_median_ns));
    println!("gpu_p99_fps={:.2}", fps(summary.gpu_p99_ns));
    Ok(())
}

fn required<'a>(values: &'a [String], index: usize, what: &str) -> Result<&'a str, io::Error> {
    values
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("missing {what}")))
}

fn component_atoms(
    structure: &pdbiox::Structure,
    component: &str,
) -> Result<(Vec<u32>, Vec<Vec3>), io::Error> {
    let Some(residue) = structure
        .data()
        .residues()
        .find(|residue| residue.name() == Some(component))
    else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("component {component} is absent"),
        ));
    };
    let indices = residue
        .atoms()
        .map(|atom| atom.index().get())
        .collect::<Vec<_>>();
    let points = indices
        .iter()
        .filter_map(|index| {
            structure
                .positions()
                .get(usize::try_from(*index).ok()?)
                .copied()
                .map(Vec3::from_array)
        })
        .collect::<Vec<_>>();
    if indices.is_empty() || points.len() != indices.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "component coordinates are absent or ragged",
        ));
    }
    Ok((indices, points))
}

fn fps(nanoseconds: u64) -> f64 {
    if nanoseconds == 0 {
        return f64::INFINITY;
    }
    let bounded = u32::try_from(nanoseconds).map_or(u32::MAX, |value| value);
    1_000_000_000.0 / f64::from(bounded)
}
