//! Publication-frame timing for one structure and one form.
//!
//! `render_image` renders a converged publication image, which accumulates
//! `QualityTier::image_samples` sub-frames, so the per-sample figure is the
//! honest cost to compare against another engine's single frame. Wall time and
//! per-sample time are both reported; neither is an interactive frame rate.

use molgfx::{Renderer, Scene, rep, sel};
use std::error::Error;
use std::io;
use std::time::Instant;

/// Samples a publication image accumulates.
const SAMPLES: f64 = 64.0;
const WARMUP: usize = 1;
const FRAMES: usize = 15;

/// The middle of a sorted sample, or NaN for an empty one.
fn middle(sorted: &[f64]) -> f64 {
    let Some(&value) = sorted.get(sorted.len() / 2) else {
        return f64::NAN;
    };
    value
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let Some(path) = arguments.next() else {
        return Err(io::Error::other("usage: frame_time STRUCTURE [FORM]").into());
    };
    let Some(form) = arguments.next() else {
        return run(&path, "cartoon");
    };
    run(&path, &form)
}

fn run(path: &str, form: &str) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let structure =
        molframe::read(path).map_err(|diagnostics| io::Error::other(format!("{diagnostics:?}")))?;
    let parse = started.elapsed();

    let started = Instant::now();
    let mut scene = Scene::from_structure(&structure)?;
    match form {
        "spacefill" => scene.add(rep::spacefill(sel::all()))?,
        "sticks" => scene.add(rep::licorice(sel::all()))?,
        "surface" => scene.add(rep::surface(sel::protein()))?,
        _ => scene.add(rep::cartoon(sel::all()))?,
    };
    let build = started.elapsed();

    let mut renderer = Renderer::new()?;
    let started = Instant::now();
    for _ in 0..WARMUP {
        let _ = renderer.render_image(&scene, (1280, 720))?;
    }
    let first = started.elapsed();
    let mut milliseconds = Vec::with_capacity(FRAMES);
    for _ in 0..FRAMES {
        let started = Instant::now();
        let _ = renderer.render_image(&scene, (1280, 720))?;
        milliseconds.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    milliseconds.sort_by(f64::total_cmp);
    let median = middle(&milliseconds);
    // One timestamped frame at the renderer's current tier: the cost of an
    // interactive frame, not of the converged publication image above.
    let mut gpu = Vec::with_capacity(FRAMES);
    for _ in 0..WARMUP + FRAMES {
        let timing = renderer.measure_frame(&scene, (1280, 720))?;
        // Timestamps are unresolved on some adapters; the blocking end-to-end
        // duration is always meaningful and is the conservative budget metric.
        gpu.push(std::time::Duration::from_nanos(timing.frame_ns).as_secs_f64() * 1000.0);
    }
    gpu.drain(..gpu.len().min(WARMUP));
    gpu.sort_by(f64::total_cmp);
    let gpu_median = middle(&gpu);
    println!(
        "{path} atoms={} form={form} parse_ms={:.1} scene_ms={:.1} first_ms={:.1} \
         publication_median_ms={median:.1} per_sample_ms={:.2} frame_ms={gpu_median:.2}",
        structure.atom_count(),
        parse.as_secs_f64() * 1000.0,
        build.as_secs_f64() * 1000.0,
        first.as_secs_f64() * 1000.0,
        median / SAMPLES,
    );
    Ok(())
}
