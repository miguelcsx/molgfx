//! Renders a two-track molecular-dynamics movie from one protein and two trajectories.
//!
//! The left track is every protein atom from an XTC file; the right track is the
//! protein's heavy atoms from a DCD file. Each track keeps its own physical clock,
//! fitted to the same playback duration, so a fast motion and a slow one are shown
//! side by side at their own time scales. On-screen times are the files' own
//! timestamps. Between saved frames the motion is linear interpolation, not
//! simulation; the caption and the manifest say so.

mod args;
mod labels;
mod manifest;
mod scene;
mod sink;
mod system;
mod track;

use std::error::Error;
use std::process::ExitCode;
use std::time::Instant;

use molgfx::profile::MeasuredOutput;
use molgfx::{Renderer, camera};
use num_traits::ToPrimitive;

use args::{Args, Quality};
use scene::Stage;

/// Burned into the bottom of every frame, and recorded in the manifest.
const FOOTER: &str = "Each track is fit to its own physical clock over the same playback time. \
Times are file timestamps; between saved frames the motion is linear interpolation, not simulation.";

fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("trajectory_video: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: impl Iterator<Item = String>) -> Result<(), Box<dyn Error>> {
    let args = Args::parse(arguments)?;
    let started = Instant::now();
    std::fs::create_dir_all(&args.out)?;

    let phase = Instant::now();
    let layout = system::load(&args.structure, &args.psf, &args.xtc, &args.dcd)?;
    let load_ms = millis(phase);
    let phase = Instant::now();
    let left = track::scan(&layout.feeds[0])?;
    let right = track::scan(&layout.feeds[1])?;
    let scan_ms = millis(phase);
    let scans = [&left, &right];
    eprintln!(
        "scanned {} left frames and {} right frames",
        left.times.len(),
        right.times.len()
    );

    let size = (args.width, args.height);
    let placements = scene::placements(scans);
    let phase = Instant::now();
    let mut stage = Stage::build(&layout, scans, placements, args.seconds)?;
    let build_ms = millis(phase);
    let (min, max) = scene::framing(scans, placements);
    let aspect = f32::from(u16::try_from(args.width)?) / f32::from(u16::try_from(args.height)?);
    let camera = camera::frame_bounds(min, max, aspect)?;

    let frames = frame_count(&args)?;
    let measured = match args.quality {
        Quality::Converged => MeasuredOutput::Converged,
        Quality::Interactive => MeasuredOutput::Interactive,
    };
    let ticks = stage.ticks(frames, args.fps);
    let titles = layout.titles();

    // Captions are written first: ffmpeg's `ass` filter reads the file when it starts.
    if args.labels {
        labels::write(
            &args.out.join("labels.ass"),
            size,
            args.fps,
            &ticks,
            &titles,
            args.seconds,
            FOOTER,
        )?;
    }

    let phase = Instant::now();
    let mut renderer = Renderer::new()?;
    let mut output = sink::Output::open(&args)?;
    let startup = manifest::Startup {
        load: load_ms,
        scan: scan_ms,
        build: build_ms,
        open: millis(phase),
    };
    let mut timings = manifest::Timings::default();
    for tick in &ticks {
        let set = Instant::now();
        stage.seek(tick)?;
        timings.set.push(millis(set));

        let render = Instant::now();
        let image = renderer.render_output_with_camera(&stage.scene, &camera, size, measured)?;
        timings.render.push(millis(render));

        let handoff = Instant::now();
        output.push(image, tick.index)?;
        timings.sink.push(millis(handoff));
    }
    output.finish()?;
    let wall_seconds = started.elapsed().as_secs_f64();

    manifest::write(
        &args.out.join("manifest.json"),
        &manifest::Run {
            args: &args,
            layout: &layout,
            scans,
            clocks: stage.clocks(),
            ticks: &ticks,
            timings: &timings,
            startup: &startup,
            caption: FOOTER,
            wall_seconds,
        },
    )?;

    eprintln!(
        "wrote {frames} frames in {wall_seconds:.1} s ({:.1} fps end to end); \
         mean set {:.2} ms, render {:.1} ms, sink {:.2} ms",
        f64::from(frames) / wall_seconds,
        manifest::mean(&timings.set),
        manifest::mean(&timings.render),
        manifest::mean(&timings.sink),
    );
    Ok(())
}

/// The number of frames in the video: playback seconds times frames per second, rounded.
fn frame_count(args: &Args) -> Result<u32, Box<dyn Error>> {
    let frames = (args.seconds * f64::from(args.fps))
        .round()
        .to_u32()
        .ok_or("the video is too long for its frame rate")?;
    if frames == 0 {
        return Err("the video would have no frames".into());
    }
    Ok(frames)
}

fn millis(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}
