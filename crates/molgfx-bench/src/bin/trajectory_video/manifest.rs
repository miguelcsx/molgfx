//! The run manifest: what each frame shows, what the run cost, and what the times
//! do and do not mean.

use std::error::Error;
use std::path::Path;

use num_traits::ToPrimitive;
use serde_json::{Value, json};

use crate::args::Args;
use crate::system::Layout;
use crate::track::{Clock, Scan, Tick};

/// Per-frame wall-clock costs in milliseconds, one entry per output frame.
#[derive(Default)]
pub(crate) struct Timings {
    /// Moving both tracks to the frame's times: frame reads and scene updates.
    pub(crate) set: Vec<f64>,
    /// Rendering the frame.
    pub(crate) render: Vec<f64>,
    /// Queueing the frame for the writer; waits only while the writer is behind.
    pub(crate) sink: Vec<f64>,
}

/// Wall-clock costs in milliseconds of each step before the first frame.
pub(crate) struct Startup {
    /// Reading the structure, selecting the atoms and pairing them with each file.
    pub(crate) load: f64,
    /// The first pass over both trajectories.
    pub(crate) scan: f64,
    /// Building the scene and opening each lane.
    pub(crate) build: f64,
    /// Creating the renderer and opening the sink.
    pub(crate) open: f64,
}

/// Everything the manifest records about a run.
pub(crate) struct Run<'a> {
    pub(crate) args: &'a Args,
    pub(crate) layout: &'a Layout,
    pub(crate) scans: [&'a Scan; 2],
    pub(crate) clocks: [Clock; 2],
    pub(crate) ticks: &'a [Tick],
    pub(crate) timings: &'a Timings,
    pub(crate) startup: &'a Startup,
    pub(crate) caption: &'a str,
    pub(crate) wall_seconds: f64,
}

/// The mean of `values`, or NaN when there are none.
pub(crate) fn mean(values: &[f64]) -> f64 {
    values
        .len()
        .to_f64()
        .map_or(f64::NAN, |count| values.iter().sum::<f64>() / count)
}

/// Writes `manifest.json` beside the video.
///
/// # Errors
/// Returns an error when the manifest cannot be serialized or written.
pub(crate) fn write(path: &Path, run: &Run<'_>) -> Result<(), Box<dyn Error>> {
    let tracks: Vec<Value> = run
        .layout
        .feeds
        .iter()
        .zip(run.scans)
        .zip(run.clocks)
        .zip(run.layout.titles())
        .map(|(((feed, scan), clock), title)| {
            json!({
                "title": title,
                "file": feed.path.display().to_string(),
                "atoms": feed.atoms,
                "frames": scan.times.len(),
                "first_time_seconds": scan.times.first(),
                "last_time_seconds": scan.times.last(),
                "clock": {
                    "first_seconds": clock.first,
                    "span_seconds": clock.span,
                    "playback_seconds": clock.playback_seconds,
                },
                "bounds": { "min": scan.min, "max": scan.max },
            })
        })
        .collect();
    let frames: Vec<Value> = run
        .ticks
        .iter()
        .map(|tick| {
            json!({
                "index": tick.index,
                "playback_seconds": tick.playback,
                "sim_seconds": tick.sim,
            })
        })
        .collect();
    let rate = run
        .ticks
        .len()
        .to_f64()
        .map_or(f64::NAN, |count| count / run.wall_seconds);
    let manifest = json!({
        "caption": run.caption,
        "honesty": "Times are file timestamps. Between saved frames the motion is linear interpolation, not simulation.",
        "structure": run.args.structure.display().to_string(),
        "xtc": run.args.xtc.display().to_string(),
        "psf": run.args.psf.display().to_string(),
        "dcd": run.args.dcd.display().to_string(),
        "video": {
            "fps": run.args.fps,
            "width": run.args.width,
            "height": run.args.height,
            "seconds": run.args.seconds,
            "frames": run.ticks.len(),
            "quality": format!("{:?}", run.args.quality),
            "sink": format!("{:?}", run.args.sink),
            "encoder": format!("{:?}", run.args.encoder),
            "labels": run.args.labels,
        },
        "tracks": tracks,
        "timings_ms": {
            "set": stage(&run.timings.set),
            "render": stage(&run.timings.render),
            "sink": stage(&run.timings.sink),
        },
        "startup_ms": {
            "load": run.startup.load,
            "scan": run.startup.scan,
            "build": run.startup.build,
            "open": run.startup.open,
        },
        "wall_seconds": run.wall_seconds,
        "frames_per_second": rate,
        "frame_times": frames,
    });
    std::fs::write(path, serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

/// Mean, maximum and total of one stage's per-frame costs.
fn stage(values: &[f64]) -> Value {
    let max = values.iter().copied().fold(0.0_f64, f64::max);
    json!({
        "mean_ms": mean(values),
        "max_ms": max,
        "total_ms": values.iter().sum::<f64>(),
    })
}
