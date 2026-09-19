//! Whole-graph benchmark for semantic ligand focus and context composition.

use molgfx::semantic::FocusScene;
use molgfx::{
    core::{AtomSelection, RepresentationKind, Scene},
    math::{BoundingSphere, Camera, Vec3},
    render::{Engine, EngineConfig, ImageConfig, RenderProfile},
};
use molgfx_bench::{CumulativeTelemetry, FrameSample, FrameSummary, summarize};
use std::error::Error;
use std::io;
use std::path::Path;
use std::time::Instant;

const WARMUP_FRAMES: usize = 20;
const MEASURED_FRAMES: usize = 120;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let path = required(&arguments, 0, "a structure path")?;
    let component = required(&arguments, 1, "a component name")?;
    let inference_start = Instant::now();
    let structure = load_structure(path)?;
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
    let counters = engine.residency_counters();
    let mut previous = CumulativeTelemetry {
        allocation_events: counters.allocation_events,
        upload_bytes: counters.upload_bytes,
        resident_bytes: counters.resident_bytes,
        stall_events: counters.stall_events,
    };
    let mut samples = Vec::with_capacity(MEASURED_FRAMES);
    for _ in 0..MEASURED_FRAMES {
        let timing = engine.profile_frame(&scene, &camera, config)?;
        let counters = timing.residency_counters();
        let current = CumulativeTelemetry {
            allocation_events: counters.allocation_events,
            upload_bytes: counters.upload_bytes,
            resident_bytes: counters.resident_bytes,
            stall_events: counters.stall_events,
        };
        samples.push(FrameSample::measured(
            timing.gpu_ns,
            timing.cpu_ns,
            timing.frame_ns,
            previous,
            current,
        )?);
        previous = current;
    }
    let summary = summarize(&samples, &mut Vec::with_capacity(samples.len()))?;
    print_summary(
        component,
        structure.atom_count(),
        [inference_ns, scene_build_ns, focus_build_ns],
        summary,
    );
    Ok(())
}

fn load_structure(path: &str) -> Result<molframe::Structure, Box<dyn Error>> {
    let (parsed, diagnostics) = molgfx_bench::reader::read_structure(Path::new(path))?;
    if let Some(diagnostics) = diagnostics {
        eprintln!("structure recovered with diagnostics: {diagnostics}");
    }
    Ok(molframe::infer_bonds(
        &parsed,
        molframe::BondInference::default(),
        &molframe::ExecutionContext::default(),
    )
    .map_err(|diagnostic| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bond inference diagnostic: {diagnostic:?}"),
        )
    })?
    .structure)
}

fn print_summary(component: &str, atoms: u32, setup_ns: [u128; 3], summary: FrameSummary) {
    println!("component={component}");
    println!("atoms={atoms}");
    println!("bond_inference_ns={}", setup_ns[0]);
    println!("scene_build_ns={}", setup_ns[1]);
    println!("focus_build_ns={}", setup_ns[2]);
    println!("frames={MEASURED_FRAMES}");
    println!("gpu_median_ns={}", summary.gpu_median_ns);
    println!("gpu_p99_ns={}", summary.gpu_p99_ns);
    println!("cpu_median_ns={}", summary.cpu_median_ns);
    println!("frame_median_ns={}", summary.frame_median_ns);
    println!("frame_p99_ns={}", summary.frame_p99_ns);
    println!("frame_p99_fps={:.2}", fps(summary.frame_p99_ns));
    println!("gpu_median_fps={:.2}", fps(summary.gpu_median_ns));
    println!("gpu_p99_fps={:.2}", fps(summary.gpu_p99_ns));
    println!("max_allocation_events={}", summary.max_allocations);
    println!("max_upload_bytes={}", summary.max_upload_bytes);
    println!("peak_resident_bytes={}", summary.peak_resident_bytes);
    println!("max_stall_events={}", summary.max_stall_events);
}

fn required<'a>(values: &'a [String], index: usize, what: &str) -> Result<&'a str, io::Error> {
    values
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("missing {what}")))
}

fn component_atoms(
    structure: &molframe::Structure,
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
    1.0 / std::time::Duration::from_nanos(nanoseconds).as_secs_f64()
}
