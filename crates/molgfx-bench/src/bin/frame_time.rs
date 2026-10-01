//! Serial completed-output measurements using the renderer's observed budget.
//! Subsamples are never counted as outputs or divided into an inferred speed.
//! GPU percentiles include only resolved timestamps. Pixel exports are excluded.
//!
//! `frame_time STRUCTURE [FORM] [--warmup-frames N] [--frames N] [--size WxH]`
//! Defaults: 120 warmup outputs, 1200 measured outputs, physical 1280x720.

use molgfx::{EffectiveQuality, FrameTiming, Renderer, Scene, profile, rep, sel};
use molgfx_bench::{CumulativeTelemetry, FrameSample, measure_heap, summarize};
use std::error::Error;
use std::io;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> = &stats_alloc::INSTRUMENTED_SYSTEM;

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1).peekable();
    let path = arguments.next().ok_or_else(|| {
        io::Error::other(
            "usage: frame_time STRUCTURE [FORM] [--warmup-frames N] [--frames N] [--size WxH]",
        )
    })?;
    let mut form = match arguments.peek() {
        Some(value) if !value.starts_with("--") => arguments.next(),
        _ => None,
    };
    let form = form.get_or_insert_with(|| String::from("cartoon"));
    let (mut warmup, mut frames, mut size) = (120_usize, 1200_usize, (1280_u32, 720_u32));
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| io::Error::other(format!("missing value for {flag}")))?;
        match flag.as_str() {
            "--warmup-frames" => warmup = value.parse()?,
            "--frames" => frames = value.parse()?,
            "--size" => {
                let (width, height) = value
                    .split_once('x')
                    .ok_or_else(|| io::Error::other("size requires WxH"))?;
                size = (width.parse()?, height.parse()?);
            }
            _ => return Err(io::Error::other(format!("unknown option: {flag}")).into()),
        }
    }
    if frames == 0 || size.0 == 0 || size.1 == 0 {
        return Err(io::Error::other("frames and physical size must be nonzero").into());
    }
    run(&path, form, warmup, frames, size)
}

fn run(
    path: &str,
    form: &str,
    warmup: usize,
    frames: usize,
    size: (u32, u32),
) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let structure =
        molframe::read(path).map_err(|diagnostics| io::Error::other(format!("{diagnostics:?}")))?;
    let parse_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let mut scene = Scene::from_structure(&structure)?;
    match form {
        "spacefill" => scene.add(rep::spacefill(sel::all()))?,
        "sticks" | "licorice" => scene.add(rep::licorice(sel::all()))?,
        "surface" => scene.add(rep::surface(sel::protein()))?,
        "cartoon" => scene.add(rep::cartoon(sel::all()))?,
        _ => return Err(io::Error::other(format!("unknown form: {form}")).into()),
    };
    let scene_ns = started.elapsed().as_nanos();
    let mut renderer = Renderer::with_profile(profile::highest_fixed(120))?;
    let mut samples = Vec::with_capacity(frames);
    let mut timings = Vec::with_capacity(frames);
    let mut pass_snapshots = Vec::with_capacity(frames);
    let mut scratch = Vec::with_capacity(frames);
    let (cold_heap, cold) = measure_heap(|| renderer.measure_frame(&scene, size));
    let cold = cold?;
    require_completed(&cold.quality, size)?;
    let cold_profile = molgfx_bench::profile_metadata_json(&cold, renderer.last_pass_timings());
    let mut previous = telemetry(&cold);
    for _ in 0..warmup {
        let timing = renderer.measure_frame(&scene, size)?;
        require_completed(&timing.quality, size)?;
        previous = telemetry(&timing);
    }
    let serial_start = Instant::now();
    for _ in 0..frames {
        let (heap, timing) = measure_heap(|| renderer.measure_frame(&scene, size));
        let timing = timing?;
        require_completed(&timing.quality, size)?;
        let current = telemetry(&timing);
        let mut sample = FrameSample::measured(
            timing.gpu_timing.nanoseconds(),
            timing.cpu_ns,
            timing.frame_ns,
            previous,
            current,
        )?;
        sample.heap = Some(heap);
        samples.push(sample);
        timings.push(timing);
        pass_snapshots.push(renderer.last_pass_timings().to_vec());
        previous = current;
    }
    let serial_elapsed = serial_start.elapsed();
    let summary = summarize(&samples, &mut scratch)?;
    for (output, ((sample, timing), passes)) in samples
        .iter()
        .zip(&timings)
        .zip(&pass_snapshots)
        .enumerate()
    {
        println!(
            "{}",
            serde_json::json!({"kind": "output", "output": output, "sample": sample, "quality": timing.quality, "profile": molgfx_bench::profile_metadata_json(timing, passes)})
        );
    }
    let outputs_per_second = num_traits::ToPrimitive::to_f64(&frames)
        .ok_or_else(|| io::Error::other("output count cannot be represented"))?
        / serial_elapsed.as_secs_f64();
    println!(
        "{}",
        serde_json::json!({
            "kind": "summary", "path": path, "atoms": structure.atom_count(), "form": form,
            "parse_ns": parse_ns, "scene_ns": scene_ns, "cold_completed_ns": cold.frame_ns,
            "cold_heap": cold_heap, "cold_quality": cold.quality,
            "cold_gpu_ns": cold.gpu_timing.nanoseconds(), "cold_gpu_timing_reason": cold.gpu_timing.unavailable_reason(),
            "cold_profile": cold_profile,
            "warmup_outputs": warmup, "summary": summary,
            "serial_completed_outputs_per_second": outputs_per_second,
            "serial_elapsed_ns": serial_elapsed.as_nanos(), "target_fps": 120,
            "latency_p95_budget_ns": 8_333_333,
            "meets_120_fps_serial": outputs_per_second >= 120.0 && summary.frame_p95_ns <= 8_333_333,
            "heap_scope": "Rust consumer/engine/backend allocator; excludes native driver allocations and pixel exports",
            "rss": null, "rss_reason": "use isolated OS process measurement; heap bytes are not RSS",
        })
    );
    Ok(())
}

fn telemetry(timing: &FrameTiming) -> CumulativeTelemetry {
    let counters = timing.residency.counters();
    CumulativeTelemetry {
        allocation_events: counters.allocation_events,
        upload_bytes: counters.upload_bytes,
        resident_bytes: counters.resident_bytes,
        stall_events: counters.stall_events,
    }
}

fn require_completed(quality: &EffectiveQuality, size: (u32, u32)) -> Result<(), io::Error> {
    if quality.extent != [size.0, size.1]
        || quality.adaptive
        || !quality.full_residency
        || !quality.complete()
        || quality.samples_required == 0
        || quality
            .samples_completed
            .is_none_or(|count| count < quality.samples_required)
    {
        return Err(io::Error::other(format!(
            "output did not satisfy fixed completed quality: {quality:?}"
        )));
    }
    Ok(())
}
