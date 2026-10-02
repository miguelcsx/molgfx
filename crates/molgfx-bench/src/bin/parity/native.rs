//! Production facade adapter; measurement excludes screenshot readback.
use super::{
    catalog::{Catalog, Fixture, Result, Style},
    script,
};
use molgfx::profile::MeasuredOutput;
use molgfx::{Color, ColorSpec, FrameTiming, PassTiming, Renderer, Scene, profile, rep, sel};
use molgfx_bench::{
    CumulativeTelemetry, FrameSample, HeapMeasurement, measure_heap, profile_metadata_json,
    summarize,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, path::Path};

pub(super) fn structure(fixture: &Fixture, cache: &Path) -> Result<(molframe::Structure, Value)> {
    if fixture.format == "mrc" {
        return Err(io::Error::other("standalone affine density requires a volume-only scene and affine VolumeBinding; current facade requires a molecular structure and only exposes origin/spacing").into());
    }
    let (structure, diagnostics) =
        molgfx_bench::reader::read_structure(&cache.join(&fixture.file))?;
    let mut orders = BTreeMap::<String, usize>::new();
    let mut provenance = BTreeMap::<String, usize>::new();
    let mut aromatic = 0;
    for bond in structure.bonds().iter() {
        *orders.entry(format!("{:?}", bond.order)).or_default() += 1;
        *provenance
            .entry(format!("{:?}", bond.provenance))
            .or_default() += 1;
        aromatic += usize::from(bond.order == molframe::BondOrder::Aromatic);
    }
    let mut secondary = BTreeMap::<String, usize>::new();
    for state in structure.secondary_structure() {
        let code = match state {
            molframe::SecondaryStructure::Unknown => "U",
            molframe::SecondaryStructure::Coil => "C",
            molframe::SecondaryStructure::Helix => "H",
            molframe::SecondaryStructure::Strand => "E",
            molframe::SecondaryStructure::Turn => "T",
        };
        *secondary.entry(code.into()).or_default() += 1;
    }
    let metadata = json!({"atoms":structure.atom_count(),"residues":structure.residue_count(),
        "bonds":structure.bonds().iter().count(),"bond_orders":orders,"aromatic_bonds":aromatic,
        "bond_provenance":provenance,"secondary_structure":secondary,
        "secondary_structure_policy":"MolFrame file assignments or explicit provider fallback","diagnostics":diagnostics});
    Ok((structure, metadata))
}

pub(super) fn render(
    catalog: &Catalog,
    fixture: &Fixture,
    cache: &Path,
    output: &Path,
    recipe: &str,
    ffmpeg: Option<&str>,
) -> Result<Value> {
    let (scene, metadata) = if fixture.script.is_some() {
        script::scene(fixture, cache)?
    } else {
        let (structure, metadata) = structure(fixture, cache)?;
        let mut scene = Scene::from_structure(&structure)?;
        add_form(&mut scene, &fixture.form, &catalog.style)?;
        (scene, metadata)
    };
    let c = &fixture.camera;
    let aspect = num_traits::ToPrimitive::to_f32(&catalog.extent[0])
        .ok_or_else(|| io::Error::other("invalid width"))?
        / num_traits::ToPrimitive::to_f32(&catalog.extent[1])
            .ok_or_else(|| io::Error::other("invalid height"))?;
    let camera = molgfx::camera::perspective(
        c.position,
        c.target,
        c.up,
        c.fov_y_degrees.to_radians(),
        aspect,
        c.near,
        c.far,
    )?;
    let kind = if recipe == "molgfx-interactive" {
        MeasuredOutput::Interactive
    } else {
        MeasuredOutput::Converged
    };
    let profile = if recipe == "molgfx-publication" {
        profile::publication()
    } else {
        profile::highest_fixed(120)
    };
    let mut renderer = Renderer::with_profile(profile)?;
    let size = (catalog.extent[0], catalog.extent[1]);
    let (heap, cold) =
        measure_heap(|| renderer.measure_frame_with_camera(&scene, &camera, size, kind));
    let cold = cold?;
    validate(&cold, catalog.extent, kind)?;
    let (_, cold_output) = record(
        &cold,
        renderer.last_pass_timings(),
        CumulativeTelemetry::default(),
        heap,
    )?;
    let mut previous = telemetry(&cold);
    let mut warmup_samples = Vec::with_capacity(catalog.warmup_outputs);
    for _ in 0..catalog.warmup_outputs {
        let (heap, timing) =
            measure_heap(|| renderer.measure_frame_with_camera(&scene, &camera, size, kind));
        let timing = timing?;
        validate(&timing, catalog.extent, kind)?;
        let (_, row) = record(&timing, renderer.last_pass_timings(), previous, heap)?;
        warmup_samples.push(row);
        previous = telemetry(&timing);
    }
    let mut samples = Vec::with_capacity(catalog.measured_outputs);
    let mut measurements = Vec::with_capacity(catalog.measured_outputs);
    let mut settings = Vec::with_capacity(catalog.measured_outputs);
    for _ in 0..catalog.measured_outputs {
        let (heap, timing) =
            measure_heap(|| renderer.measure_frame_with_camera(&scene, &camera, size, kind));
        let timing = timing?;
        validate(&timing, catalog.extent, kind)?;
        let (sample, row) = record(&timing, renderer.last_pass_timings(), previous, heap)?;
        measurements.push(sample);
        samples.push(row);
        settings.push(serde_json::to_value(timing.quality)?);
        previous = telemetry(&timing);
    }
    let image = renderer.render_output_with_camera(&scene, &camera, size, kind)?;
    if kind == MeasuredOutput::Converged && !image.quality().complete() {
        return Err(io::Error::other("incomplete native screenshot").into());
    }
    std::fs::write(output.join("image.png"), image.png_bytes()?)?;
    if recipe == "molgfx-publication"
        && let (Some(video), Some(directory)) = (
            fixture.script.as_ref().and_then(|s| s.video.as_ref()),
            output.parent(),
        )
    {
        script::frames(catalog, fixture, &scene, &mut renderer, video, directory)?;
        script::encode(ffmpeg, video, directory)?;
    }
    let mut scratch = Vec::with_capacity(measurements.len());
    let summary = summarize(&measurements, &mut scratch)?;
    Ok(
        json!({"engine":"molgfx","engine_version":env!("CARGO_PKG_VERSION"),"metadata":metadata,
        "cold_output":cold_output,"warmup_samples":warmup_samples,"samples":samples,
        "effective_settings":settings,"image":"image.png","summary":summary,
        "cpu_timing_scope":"host construction elapsed time; preparation, recording and submission, not exclusive CPU utilization",
        "measurement_scope":"completed production exposure; PNG/readback excluded; cold/warm phases excluded from measured percentiles",
        "physical_settings_equivalent":false,
        "settings_difference":"Facade fixes lighting/environment/radii/effect recipes; external rigs and transport are not identical; no speedup is computed"}),
    )
}

fn add_form(scene: &mut Scene, form: &str, style: &Style) -> Result<()> {
    let [r, g, b] = style.color_rgb;
    let color = ColorSpec::Uniform {
        color: Color::rgb(r, g, b),
    };
    match form {
        "spacefill" => scene.add(
            rep::spacefill(sel::all())
                .radius(style.atom_radius_scale)
                .color(color)
                .opacity(style.opacity),
        )?,
        "ball_and_stick" => scene.add(
            rep::ball_and_stick(sel::all())
                .radius(style.atom_radius_scale)
                .bond_radius(style.bond_radius_angstrom)
                .color(color)
                .opacity(style.opacity),
        )?,
        "cartoon" => scene.add(
            rep::cartoon(sel::all())
                .width(style.cartoon_width_angstrom)
                .color(color)
                .opacity(style.opacity),
        )?,
        _ => return Err(io::Error::other("unsupported molecular parity form").into()),
    };
    Ok(())
}

fn validate(timing: &FrameTiming, extent: [u32; 2], kind: MeasuredOutput) -> Result<()> {
    let quality = &timing.quality;
    let satisfied = match kind {
        MeasuredOutput::Converged => quality.complete(),
        MeasuredOutput::Interactive => quality.samples_completed == Some(1),
    };
    if !satisfied || quality.adaptive || quality.extent != extent {
        return Err(io::Error::other(format!("incomplete native exposure: {quality:?}")).into());
    }
    Ok(())
}

fn record(
    timing: &FrameTiming,
    passes: &[PassTiming],
    previous: CumulativeTelemetry,
    heap: HeapMeasurement,
) -> Result<(FrameSample, Value)> {
    let mut sample = FrameSample::measured(
        timing.gpu_timing.nanoseconds(),
        timing.cpu_ns,
        timing.frame_ns,
        previous,
        telemetry(timing),
    )?;
    sample.heap = Some(heap);
    let mut row = profile_metadata_json(timing, passes);
    row["sample"] = serde_json::to_value(sample)?;
    row["gpu_unavailable_reason"] = json!(timing.gpu_timing.unavailable_reason());
    row["effective_settings"] = serde_json::to_value(timing.quality)?;
    Ok((sample, row))
}

fn telemetry(timing: &FrameTiming) -> CumulativeTelemetry {
    let c = timing.residency.counters();
    CumulativeTelemetry {
        allocation_events: c.allocation_events,
        upload_bytes: c.upload_bytes,
        resident_bytes: c.resident_bytes,
        stall_events: c.stall_events,
    }
}
