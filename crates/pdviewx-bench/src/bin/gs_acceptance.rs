//! Declarative GS-001..GS-010 candidate and telemetry runner.

use pdviewx::{
    AtomSelection, AttributeColumn, AttributeValues, Camera, Engine, EngineConfig, Mat4,
    RenderProfile, RepresentationKind, RowDomain, ScalarRamp, Scene, Select, SurfaceKind, Vec3,
    VisualProgramBuilder, VisualStyle,
};
use pdviewx_recipes::{AtomCorrespondence, DifferenceScene, DifferenceStyle, FocusScene};
#[path = "../acceptance/mod.rs"]
mod acceptance;
use acceptance::{
    AcceptanceReport, AdapterEvidence, Availability, CandidateEvidence, GoldenSceneId,
    GoldenSceneSpec, MetricSet, SceneEvidence, SceneOutcome, SceneRecipe, arguments,
    dimension_aspect, golden_scene_specs, read_structure, write_png,
};
use pdviewx_bench::{CumulativeTelemetry, FrameSample, summarize};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::alloc::System;
use std::cell::Cell;
use std::error::Error;
use std::fs::File;
use std::io;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = arguments()?;
    std::fs::create_dir_all(&arguments.output)?;
    let specs = golden_scene_specs();
    let selected = specs
        .iter()
        .filter(|spec| {
            arguments
                .scene
                .as_deref()
                .is_none_or(|id| id == spec.id.as_str())
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "unknown GS scene id").into());
    }
    let mut evidence = Vec::with_capacity(selected.len());
    let mut adapter = None;
    for spec in selected {
        let fixture = arguments
            .fixture
            .clone()
            .or_else(|| spec.default_fixture.map(PathBuf::from));
        let (scene_evidence, observed_adapter) = run_scene(
            spec,
            fixture,
            &arguments.output,
            arguments.warmup,
            arguments.frames,
        );
        if adapter.is_none() {
            adapter = observed_adapter;
        }
        evidence.push(scene_evidence);
    }
    let report = AcceptanceReport {
        schema_version: 1,
        evidence_scope: "single-adapter candidate; never cross-adapter or accepted golden evidence",
        adapter: match adapter {
            Some(value) => value,
            None => unavailable_adapter(),
        },
        scenes: evidence,
    };
    let report_path = arguments.output.join("acceptance-report.json");
    serde_json::to_writer_pretty(File::create(&report_path)?, &report)?;
    println!("report={}", report_path.display());
    Ok(())
}

fn run_scene(
    spec: &GoldenSceneSpec,
    fixture: Option<PathBuf>,
    output: &Path,
    warmup: usize,
    frames: usize,
) -> (SceneEvidence, Option<AdapterEvidence>) {
    let Some(fixture) = fixture else {
        return (blocked(spec, None, spec.fixture_requirement), None);
    };
    let observed_atoms = Cell::new(None);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        execute_scene(spec, &fixture, output, warmup, frames, &observed_atoms)
    }));
    match result {
        Ok(Ok((mut evidence, adapter))) => {
            add_contract_blockers(spec, &mut evidence.blockers);
            (evidence, Some(adapter))
        }
        Ok(Err(error)) => (
            failed(
                spec,
                &fixture,
                observed_atoms.get(),
                &format!("{}: {error}", spec.id.as_str()),
            ),
            None,
        ),
        Err(payload) => (
            failed(
                spec,
                &fixture,
                observed_atoms.get(),
                &format!(
                    "{}: renderer panicked: {}",
                    spec.id.as_str(),
                    panic_reason(payload.as_ref())
                ),
            ),
            None,
        ),
    }
}

fn execute_scene(
    spec: &GoldenSceneSpec,
    fixture: &Path,
    output: &Path,
    warmup: usize,
    frames: usize,
    observed: &Cell<Option<u64>>,
) -> Result<(SceneEvidence, AdapterEvidence), Box<dyn Error>> {
    let (structure, diagnostics) = read_structure(fixture)?;
    let observed_atoms = u64::from(structure.atom_count());
    observed.set(Some(observed_atoms));
    let scene = build_scene(spec.recipe, &structure, fixture)?;
    let camera = Camera::framing_aabb(
        &scene.world_aabb(),
        dimension_aspect(spec.config.width, spec.config.height)?,
    );
    let mut engine = Engine::new(
        &EngineConfig {
            mode: spec.mode,
            profile: profile(spec.recipe),
            ..EngineConfig::default()
        },
        None,
    )?;
    let adapter = AdapterEvidence {
        scope: "single-adapter",
        capability_fingerprint: format!("{:?}", engine.capabilities()),
        runtime_name: Availability::Unavailable {
            reason: "backend-neutral facade exposes capabilities but no adapter name".into(),
        },
    };
    let candidate_path = output.join(format!("{}-candidate.png", spec.id.as_str().to_lowercase()));
    let _stabilization = engine.render_image(&scene, &camera, spec.config)?;
    let image = engine.render_image(&scene, &camera, spec.config)?;
    let repeated = engine.render_image(&scene, &camera, spec.config)?;
    let repeated_bytes_equal = image.pixels == repeated.pixels;
    write_png(&candidate_path, &image)?;
    let repeated_path = output.join(format!("{}-repeat.png", spec.id.as_str().to_lowercase()));
    if !repeated_bytes_equal {
        write_png(&repeated_path, &repeated)?;
    }
    let mut blockers = Vec::new();
    if let Some(diagnostics) = diagnostics {
        eprintln!(
            "{} fixture recovered with diagnostics: {diagnostics}",
            spec.id.as_str()
        );
        blockers.push(format!("fixture recovered with diagnostics: {diagnostics}"));
    }
    let metrics = match measure(&mut engine, &scene, &camera, spec, warmup, frames) {
        Ok(value) => value,
        Err(error) => {
            let reason = format!("profiling unavailable: {error}");
            blockers.push(reason.clone());
            MetricSet::unavailable(frames, &reason)
        }
    };
    if outside_target(observed_atoms, spec.target_atoms) {
        blockers.push(format!(
            "fixture scale mismatch: observed {observed_atoms} atoms; target is approximately {}",
            spec.target_atoms
        ));
    }
    if !repeated_bytes_equal {
        blockers.push("two stabilized publication renders were not byte-identical".into());
    }
    if let (Some(gate_fps), Availability::Available { value, .. }) =
        (spec.gate_fps, &metrics.frame_p99_ns)
    {
        let budget_ns = 1_000_000_000_u64 / u64::from(gate_fps);
        if *value > budget_ns {
            blockers.push(format!(
                "frame p99 gate failed: {value} ns exceeds {budget_ns} ns ({gate_fps} fps)"
            ));
        }
    }
    Ok((
        SceneEvidence {
            id: spec.id,
            title: spec.title,
            recipe: spec.recipe,
            outcome: SceneOutcome::Candidate,
            fixture: Some(fixture.display().to_string()),
            target_atoms: spec.target_atoms,
            observed_atoms: Some(observed_atoms),
            blockers,
            candidate: Some(CandidateEvidence {
                path: candidate_path.display().to_string(),
                width: image.width,
                height: image.height,
                stabilization_renders: 1,
                repeated_bytes_equal,
                repeated_path_on_mismatch: (!repeated_bytes_equal)
                    .then(|| repeated_path.display().to_string()),
                accepted_reference: Availability::Unavailable {
                    reason: "no accepted GS reference is assigned; candidate was not rebaselined"
                        .into(),
                },
            }),
            metrics: Some(metrics),
        },
        adapter,
    ))
}

fn build_scene(
    recipe: SceneRecipe,
    structure: &pdbiox::Structure,
    fixture: &Path,
) -> Result<Scene, Box<dyn Error>> {
    let mut scene = Scene::from_structure(structure)?;
    match recipe {
        SceneRecipe::FocusPocket | SceneRecipe::QualityPocket => focus_pocket(&mut scene)?,
        SceneRecipe::CartoonLigand | SceneRecipe::Publication4k => cartoon_ligand(&mut scene)?,
        SceneRecipe::Spacefill | SceneRecipe::CapsidRegion | SceneRecipe::SemanticLod => {
            represent_all(&mut scene, RepresentationKind::Spacefill)?;
        }
        SceneRecipe::TransparentSurface => transparent_surface(&mut scene)?,
        SceneRecipe::Confidence => confidence(&mut scene, structure, fixture)?,
        SceneRecipe::Difference => difference(&mut scene, structure)?,
    }
    Ok(scene)
}

fn focus_pocket(scene: &mut Scene) -> Result<(), Box<dyn Error>> {
    let ligand = scene.select(Select::ligands())?;
    scene.focus(ligand)?;
    Ok(())
}

fn cartoon_ligand(scene: &mut Scene) -> Result<(), Box<dyn Error>> {
    let polymer = scene.select(Select::polymer())?;
    scene.represent(polymer, RepresentationKind::Cartoon)?;
    let ligand = scene.select(Select::ligands())?;
    scene.represent(ligand, RepresentationKind::BallAndStick)?;
    Ok(())
}

fn represent_all(scene: &mut Scene, kind: RepresentationKind) -> Result<(), Box<dyn Error>> {
    let all = scene.add_selection(AtomSelection::All);
    scene.represent(all, kind)?;
    Ok(())
}

fn transparent_surface(scene: &mut Scene) -> Result<(), Box<dyn Error>> {
    cartoon_ligand(scene)?;
    let all = scene.add_selection(AtomSelection::All);
    let handle = scene.represent(all, RepresentationKind::Surface)?;
    let representation = scene
        .representation_mut(handle)
        .ok_or_else(|| io::Error::other("surface representation became stale"))?;
    representation.params.surface_kind = SurfaceKind::SolventExcluded;
    representation.material.opacity = 0.42;
    Ok(())
}

fn confidence(
    scene: &mut Scene,
    structure: &pdbiox::Structure,
    fixture: &Path,
) -> Result<(), Box<dyn Error>> {
    let owner = scene
        .structures()
        .next()
        .map(|(handle, _)| handle)
        .ok_or_else(|| io::Error::other("confidence scene has no structure"))?;
    let values = structure
        .data()
        .atoms()
        .map(|atom| atom.b_factor().map_or(f32::NAN, |value| value))
        .collect::<Vec<_>>();
    let domain = display_domain(&values)?;
    let attribute = scene.add_attribute(AttributeColumn::new(
        RowDomain::Atoms(owner),
        "pLDDT",
        AttributeValues::Scalar(Arc::from(values)),
    )?)?;
    let selection = scene.add_structure_selection(owner, AtomSelection::All)?;
    let handle = scene.represent(selection, RepresentationKind::Cartoon)?;
    let mut builder = VisualProgramBuilder::new();
    let value = builder.scalar_attribute(attribute)?;
    let color = builder.ramp(value, ScalarRamp::sequential(domain))?;
    let low = builder.scalar(domain[0])?;
    let high = builder.scalar(domain[1])?;
    let confidence = builder.smoothstep(low, high, value)?;
    let minimum = builder.scalar(0.12)?;
    let maximum = builder.scalar(1.0)?;
    let opacity = builder.mix_scalar(minimum, maximum, confidence)?;
    builder.set_base_color(color)?;
    builder.set_opacity(opacity)?;
    let representation = scene
        .representation_mut(handle)
        .ok_or_else(|| io::Error::other("confidence representation became stale"))?;
    representation.visual = Some(VisualStyle::new(builder.finish()?));
    let _ = fixture;
    Ok(())
}

fn display_domain(values: &[f32]) -> Result<[f32; 2], io::Error> {
    let mut finite = values.iter().copied().filter(|value| value.is_finite());
    let Some(first) = finite.next() else {
        return Err(io::Error::other("attribute has no finite values"));
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

fn difference(scene: &mut Scene, structure: &pdbiox::Structure) -> Result<(), Box<dyn Error>> {
    let left = scene
        .structures()
        .next()
        .map(|(handle, _)| handle)
        .ok_or_else(|| io::Error::other("difference scene has no first placement"))?;
    let right = scene.add_structure(structure)?;
    let placed = scene
        .structure_mut(right)
        .ok_or_else(|| io::Error::other("difference placement became stale"))?;
    placed.model_to_world = Mat4::from_translation(Vec3::new(1.2, 0.0, 0.0));
    let count = structure.atom_count();
    let correspondence = (0..count)
        .map(|row| AtomCorrespondence {
            left: row,
            right: row,
        })
        .collect::<Vec<_>>();
    scene.render_difference(
        [left, right],
        &correspondence,
        Arc::<str>::from("GS-009 topology-identical row correspondence"),
        DifferenceStyle::default(),
    )?;
    Ok(())
}

fn measure(
    engine: &mut Engine,
    scene: &Scene,
    camera: &Camera,
    spec: &GoldenSceneSpec,
    warmup: usize,
    frames: usize,
) -> Result<MetricSet, Box<dyn Error>> {
    if frames == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "measured frame count must be positive",
        )
        .into());
    }
    for _ in 0..warmup {
        engine.profile_frame(scene, camera, spec.config)?;
    }
    let mut previous = telemetry(engine);
    let mut samples = Vec::with_capacity(frames);
    let mut gpu_samples_resolved = true;
    let mut max_process_heap_allocations = 0_u64;
    for _ in 0..frames {
        let allocation_region = Region::new(GLOBAL);
        let timing = engine.profile_frame(scene, camera, spec.config)?;
        let allocation_change = allocation_region.change();
        let allocation_events = allocation_change
            .allocations
            .checked_add(allocation_change.reallocations)
            .ok_or_else(|| io::Error::other("process allocation event count overflow"))?;
        max_process_heap_allocations = max_process_heap_allocations.max(
            u64::try_from(allocation_events)
                .map_err(|_| io::Error::other("process allocation event count exceeds u64"))?,
        );
        gpu_samples_resolved &= timing.gpu_timing_resolved;
        let value = timing.residency_counters();
        let current = CumulativeTelemetry {
            allocation_events: value.allocation_events,
            upload_bytes: value.upload_bytes,
            resident_bytes: value.resident_bytes,
            stall_events: value.stall_events,
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
    let summary = summarize(&samples, &mut Vec::with_capacity(frames))?;
    Ok(MetricSet::measured(
        frames,
        summary,
        gpu_samples_resolved,
        max_process_heap_allocations,
    ))
}

fn telemetry(engine: &Engine) -> CumulativeTelemetry {
    let value = engine.residency_counters();
    CumulativeTelemetry {
        allocation_events: value.allocation_events,
        upload_bytes: value.upload_bytes,
        resident_bytes: value.resident_bytes,
        stall_events: value.stall_events,
    }
}

fn add_contract_blockers(spec: &GoldenSceneSpec, blockers: &mut Vec<String>) {
    if matches!(spec.id, GoldenSceneId::Gs001 | GoldenSceneId::Gs007) {
        blockers.push(
            "provider interaction and label payloads are absent from the local fixture route"
                .into(),
        );
    }
    if let Some(limit) = spec.initial_frame_limit_ms {
        blockers.push(format!(
            "first improved frame and convergence are not instrumented for the {limit} ms gate"
        ));
    }
    blockers.push("cross-adapter differential evidence is unavailable".into());
    blockers.push("accepted perceptual reference is unavailable".into());
}

fn profile(recipe: SceneRecipe) -> RenderProfile {
    if matches!(recipe, SceneRecipe::Publication4k) {
        RenderProfile::cinematic()
    } else {
        RenderProfile::inspection()
    }
}

fn outside_target(observed: u64, target: u64) -> bool {
    observed < target.saturating_mul(3) / 4 || observed > target.saturating_mul(5) / 4
}

include!("gs_acceptance/outcomes.rs");
