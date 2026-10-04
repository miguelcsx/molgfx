//! Production facade adapter; measurement excludes screenshot readback.
use super::script;
use molgfx::profile::MeasuredOutput;
use molgfx::{FrameTiming, PassTiming, Renderer};
use molgfx_bench::gallery::{self, Catalog, Fixture, Result};
use molgfx_bench::{
    CumulativeTelemetry, FrameSample, HeapMeasurement, measure_heap, profile_metadata_json,
    summarize,
};
use serde_json::{Value, json};
use std::{io, path::Path};

pub(super) fn render(
    catalog: &Catalog,
    fixture: &Fixture,
    cache: &Path,
    output: &Path,
    recipe: &str,
    ffmpeg: Option<&str>,
) -> Result<Value> {
    let (scene, metadata) = gallery::scene(fixture, cache, &catalog.style)?;
    let camera = gallery::camera(catalog.extent, &fixture.camera)?;
    let kind = match recipe {
        "molgfx-interactive" => MeasuredOutput::Interactive,
        "molgfx-converged" => MeasuredOutput::Converged,
        _ => return Err(io::Error::other(format!("unknown native recipe: {recipe}")).into()),
    };
    let profile = gallery::profile(fixture, kind)?;
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
    validate_pixels(image.pixels())?;
    std::fs::write(output.join("image.png"), image.png_bytes()?)?;
    if recipe == "molgfx-converged"
        && let (Some(video), Some(directory)) = (
            fixture.script.as_ref().and_then(|s| s.video.as_ref()),
            output.parent(),
        )
    {
        script::frames(
            catalog,
            fixture,
            cache,
            &scene,
            &mut renderer,
            video,
            directory,
        )?;
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

fn validate_pixels(pixels: &[u8]) -> Result<()> {
    if pixels.is_empty() || pixels.iter().all(|byte| *byte == 0) {
        return Err(io::Error::other("native gallery capture contains no visible pixels").into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
