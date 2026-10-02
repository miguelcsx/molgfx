//! Serial completed-output measurements using the renderer's observed budget.
//! Subsamples are never counted as outputs or divided into an inferred speed.
//! GPU percentiles include only resolved timestamps. Pixel exports are excluded.
//!
//! ```text
//! frame_time STRUCTURE [FORM] [options]       ad-hoc file
//! frame_time --case L<n> --cache DIR [options] ladder scene (see ladder.rs)
//! options: --recipe interactive|converged  --orbit DEG  --warmup-frames N
//!          --frames N  --size WxH
//! ```
//! Defaults: 120 warmup outputs, 1200 measured outputs, 1280x720 for files and
//! 1920x1080 for ladder scenes. `interactive` renders one single-sample frame
//! per output with the camera yawing `--orbit` degrees per output (0.5 by
//! default); `converged` renders the full publication exposure on a still camera.

use molgfx::command::Session;
use molgfx::profile::MeasuredOutput;
use molgfx::{EffectiveQuality, FrameTiming, Renderer, Scene, profile, rep, sel};
use molgfx_bench::ladder::{self, LadderCase, LadderQuality};
use molgfx_bench::{CumulativeTelemetry, FrameSample, fallback, measure_heap, summarize};
use num_traits::ToPrimitive;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> = &stats_alloc::INSTRUMENTED_SYSTEM;

const USAGE: &str = "usage: frame_time (STRUCTURE [FORM] | --case L<n> --cache DIR) \
[--recipe interactive|converged] [--orbit DEG] [--warmup-frames N] [--frames N] [--size WxH]";
const SECOND_NS: f64 = 1.0e9;

struct Options {
    source: Source,
    output: MeasuredOutput,
    orbit_degrees: f32,
    warmup: usize,
    frames: usize,
    size: (u32, u32),
}

enum Source {
    File {
        path: String,
        form: String,
    },
    Ladder {
        case: &'static LadderCase,
        cache: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    run(&parse()?)
}

fn parse() -> Result<Options, Box<dyn Error>> {
    let usage = || io::Error::other(USAGE);
    let mut arguments = std::env::args().skip(1).peekable();
    let mut positional = Vec::new();
    while let Some(value) = arguments.next_if(|value| !value.starts_with("--")) {
        positional.push(value);
    }
    let (mut case, mut cache) = (None, None);
    let mut output = MeasuredOutput::Converged;
    let mut orbit = None;
    let (mut warmup, mut frames, mut size) = (120_usize, 1200_usize, None);
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| io::Error::other(format!("missing value for {flag}")))?;
        match flag.as_str() {
            "--case" => case = Some(value),
            "--cache" => cache = Some(PathBuf::from(value)),
            "--recipe" => {
                output = match value.as_str() {
                    "interactive" => MeasuredOutput::Interactive,
                    "converged" => MeasuredOutput::Converged,
                    _ => return Err(io::Error::other("recipe is interactive or converged").into()),
                };
            }
            "--orbit" => orbit = Some(value.parse::<f32>()?),
            "--warmup-frames" => warmup = value.parse()?,
            "--frames" => frames = value.parse()?,
            "--size" => {
                let (width, height) = value
                    .split_once('x')
                    .ok_or_else(|| io::Error::other("size requires WxH"))?;
                size = Some((width.parse()?, height.parse()?));
            }
            _ => return Err(io::Error::other(format!("unknown option: {flag}")).into()),
        }
    }
    let source = if let Some(id) = case {
        let case = ladder::find(&id)
            .ok_or_else(|| io::Error::other(format!("unknown ladder case {id}")))?;
        let cache = cache.ok_or_else(|| io::Error::other("--case requires --cache DIR"))?;
        Source::Ladder { case, cache }
    } else {
        let mut positional = positional.into_iter();
        let path = positional.next().ok_or_else(usage)?;
        let form = fallback(positional.next(), String::from("cartoon"));
        Source::File { path, form }
    };
    let default_size = if matches!(source, Source::Ladder { .. }) {
        (1920, 1080)
    } else {
        (1280, 720)
    };
    let size = fallback(size, default_size);
    let interactive = output == MeasuredOutput::Interactive;
    let orbit_degrees = fallback(orbit, if interactive { 0.5 } else { 0.0 });
    if frames == 0 || size.0 == 0 || size.1 == 0 || !orbit_degrees.is_finite() {
        return Err(io::Error::other("frames, physical size and orbit must be valid").into());
    }
    Ok(Options {
        source,
        output,
        orbit_degrees,
        warmup,
        frames,
        size,
    })
}

fn build_scene(source: &Source) -> Result<(Scene, molframe::Structure, String), Box<dyn Error>> {
    let (path, form) = match source {
        Source::File { path, form } => (PathBuf::from(path), form.as_str()),
        Source::Ladder { case, cache } => {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("parity/corpus.json");
            (
                ladder::fixture_path(&manifest, cache, case.fixture)?,
                "ladder",
            )
        }
    };
    let structure = molframe::read(&path)
        .map_err(|diagnostics| io::Error::other(format!("{diagnostics:?}")))?;
    let mut scene = Scene::from_structure(&structure)?;
    if let Source::Ladder { case, .. } = source {
        let mut session = Session::new(&scene);
        for line in case.commands {
            session
                .execute_text(&mut scene, line)
                .map_err(|errors| io::Error::other(errors.render(line)))?;
        }
    } else {
        match form {
            "spacefill" => scene.add(rep::spacefill(sel::all()))?,
            "sticks" | "licorice" => scene.add(rep::licorice(sel::all()))?,
            "surface" => scene.add(rep::surface(sel::protein()))?,
            "cartoon" => scene.add(rep::cartoon(sel::all()))?,
            _ => return Err(io::Error::other(format!("unknown form: {form}")).into()),
        };
    }
    Ok((scene, structure, form.to_owned()))
}

/// The framing camera yawed about its up axis through its target by `degrees`.
fn orbited(base: &molgfx::Camera, degrees: f32) -> molgfx::Camera {
    let offset = base.eye - base.target;
    let (sin, cos) = degrees.to_radians().sin_cos();
    let up = base.up.normalize_or_zero();
    // Rodrigues rotation of the eye offset about the up axis.
    let rotated = offset * cos + up.cross(offset) * sin + up * (up.dot(offset) * (1.0 - cos));
    let mut camera = *base;
    camera.eye = base.target + rotated;
    camera
}

fn aspect(size: (u32, u32)) -> Result<f32, io::Error> {
    (f64::from(size.0) / f64::from(size.1))
        .to_f32()
        .ok_or_else(|| io::Error::other("aspect ratio is not representable"))
}

impl Options {
    /// Quality policy, frame-rate budget and the renderer profile implementing it.
    fn policy(&self) -> (LadderQuality, u32, profile::RenderProfile) {
        let (quality, budget_fps) = match &self.source {
            Source::Ladder { case, .. } => (case.quality, case.budget_fps),
            Source::File { .. } => (LadderQuality::HighestFixed, 120),
        };
        let profile = match quality {
            LadderQuality::HighestFixed => profile::highest_fixed(120),
            LadderQuality::Auto => profile::adaptive(30),
        };
        (quality, budget_fps, profile)
    }
}

fn print_outputs(
    recipe: &str,
    samples: &[FrameSample],
    timings: &[FrameTiming],
    passes: &[Vec<molgfx::PassTiming>],
) {
    for (index, ((sample, timing), passes)) in samples.iter().zip(timings).zip(passes).enumerate() {
        println!(
            "{}",
            serde_json::json!({"kind": "output", "recipe": recipe, "output": index, "sample": sample, "quality": timing.quality, "profile": molgfx_bench::profile_metadata_json(timing, passes)})
        );
    }
}

fn run(options: &Options) -> Result<(), Box<dyn Error>> {
    let Options {
        output,
        orbit_degrees,
        warmup,
        frames,
        size,
        ..
    } = *options;
    let started = Instant::now();
    let (scene, structure, form) = build_scene(&options.source)?;
    let scene_ns = started.elapsed().as_nanos();
    let (quality, budget_fps, profile) = options.policy();
    let mut renderer = Renderer::with_profile(profile)?;
    let base = scene.framing_camera(aspect(size)?);
    // Exact in f32 for the first 16M outputs, far beyond any run here.
    let mut outputs_rendered = 0.0_f32;
    let mut next_camera = || {
        let camera = orbited(&base, orbit_degrees * outputs_rendered);
        outputs_rendered += 1.0;
        camera
    };
    let mut samples = Vec::with_capacity(frames);
    let mut timings = Vec::with_capacity(frames);
    let mut pass_snapshots = Vec::with_capacity(frames);
    let mut scratch = Vec::with_capacity(frames);
    let camera = next_camera();
    let (cold_heap, cold) =
        measure_heap(|| renderer.measure_frame_with_camera(&scene, &camera, size, output));
    let cold = cold?;
    require_completed(&cold.quality, size, output, quality)?;
    let cold_profile = molgfx_bench::profile_metadata_json(&cold, renderer.last_pass_timings());
    let mut previous = telemetry(&cold);
    for _ in 0..warmup {
        let camera = next_camera();
        let timing = renderer.measure_frame_with_camera(&scene, &camera, size, output)?;
        require_completed(&timing.quality, size, output, quality)?;
        previous = telemetry(&timing);
    }
    let serial_start = Instant::now();
    for _ in 0..frames {
        let camera = next_camera();
        let (heap, timing) =
            measure_heap(|| renderer.measure_frame_with_camera(&scene, &camera, size, output));
        let timing = timing?;
        require_completed(&timing.quality, size, output, quality)?;
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
    let recipe = if output == MeasuredOutput::Interactive {
        "interactive"
    } else {
        "converged"
    };
    print_outputs(recipe, &samples, &timings, &pass_snapshots);
    let outputs_per_second = frames
        .to_f64()
        .ok_or_else(|| io::Error::other("output count cannot be represented"))?
        / serial_elapsed.as_secs_f64();
    let budget_ns = SECOND_NS / f64::from(budget_fps);
    let (case, path) = match &options.source {
        Source::Ladder { case, .. } => (Some(case.id), None),
        Source::File { path, .. } => (None, Some(path.as_str())),
    };
    println!(
        "{}",
        serde_json::json!({
            "kind": "summary", "recipe": recipe, "case": case, "path": path,
            "atoms": structure.atom_count(), "form": form, "size": [size.0, size.1],
            "orbit_degrees_per_output": orbit_degrees,
            "scene_ns": scene_ns, "cold_completed_ns": cold.frame_ns,
            "cold_heap": cold_heap, "cold_quality": cold.quality,
            "cold_gpu_ns": cold.gpu_timing.nanoseconds(), "cold_gpu_timing_reason": cold.gpu_timing.unavailable_reason(),
            "cold_profile": cold_profile,
            "warmup_outputs": warmup, "summary": summary,
            "p50_ns": summary.frame_median_ns, "p95_ns": summary.frame_p95_ns, "p99_ns": summary.frame_p99_ns,
            "gpu_resolved_count": summary.gpu_resolved_count,
            "heap_allocations_per_output": summary.max_heap_allocations,
            "serial_completed_outputs_per_second": outputs_per_second,
            "serial_elapsed_ns": serial_elapsed.as_nanos(),
            "target_fps": budget_fps, "latency_p95_budget_ns": budget_ns,
            "meets_budget": outputs_per_second >= f64::from(budget_fps)
                && summary.frame_p95_ns.to_f64().is_some_and(|p95| p95 <= budget_ns),
            "heap_scope": "Rust consumer/engine/backend allocator; excludes native driver allocations and pixel exports",
            "rss": null, "rss_scope": "measure with /usr/bin/time -l around the process",
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

fn require_completed(
    quality: &EffectiveQuality,
    size: (u32, u32),
    output: MeasuredOutput,
    policy: LadderQuality,
) -> Result<(), io::Error> {
    let extent = quality.extent == [size.0, size.1];
    let satisfied = match output {
        // One in-flight single-sample frame is what interaction presents.
        MeasuredOutput::Interactive => {
            quality.samples_completed == Some(1)
                && (policy == LadderQuality::Auto || !quality.adaptive)
        }
        MeasuredOutput::Converged => {
            !quality.adaptive
                && quality.full_residency
                && quality.complete()
                && quality.samples_required > 0
                && quality
                    .samples_completed
                    .is_some_and(|count| count >= quality.samples_required)
        }
    };
    if !extent || !satisfied {
        return Err(io::Error::other(format!(
            "output did not satisfy fixed completed quality: {quality:?}"
        )));
    }
    Ok(())
}
