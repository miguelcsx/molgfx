//! Pick cost on an idle queue and behind a frame in flight.
//!
//! ```text
//! pick_latency [--residues N] [--size WxH] [--picks N] [--warmup N]
//! ```
//!
//! Each scenario reports nanosecond percentiles of the blocking `pick` call and
//! the heap allocations one call makes. `idle` waits for the previous frame
//! before it asks; `in_flight` asks straight after submitting a frame, which is
//! the case a hover meets on every pointer move. `stream` asks through
//! `request_pick`, drives `poll` until the answer arrives and takes it; its
//! allocation count covers the request, every poll and the take.

use molgfx::{PickStream, Renderer, Scene, StreamPick, profile, rep, sel};
use molgfx_bench::{HeapMeasurement, measure_heap, percentile, synthetic};
use std::error::Error;
use std::io;
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> = &stats_alloc::INSTRUMENTED_SYSTEM;

const USAGE: &str = "usage: pick_latency [--residues N] [--size WxH] [--picks N] [--warmup N]";

struct Options {
    residues: usize,
    size: (u32, u32),
    picks: usize,
    warmup: usize,
}

fn parse() -> Result<Options, Box<dyn Error>> {
    let mut options = Options {
        residues: 4000,
        size: (1920, 1080),
        picks: 500,
        warmup: 50,
    };
    let mut arguments = std::env::args().skip(1);
    while let Some(flag) = arguments.next() {
        let value = arguments.next().ok_or_else(|| io::Error::other(USAGE))?;
        match flag.as_str() {
            "--residues" => options.residues = value.parse()?,
            "--picks" => options.picks = value.parse()?,
            "--warmup" => options.warmup = value.parse()?,
            "--size" => {
                let (width, height) = value
                    .split_once('x')
                    .ok_or_else(|| io::Error::other("size requires WxH"))?;
                options.size = (width.parse()?, height.parse()?);
            }
            _ => return Err(io::Error::other(USAGE).into()),
        }
    }
    if options.picks == 0 || options.size.0 == 0 || options.size.1 == 0 {
        return Err(io::Error::other("picks and size must be non-zero").into());
    }
    Ok(options)
}

fn report(scenario: &str, samples: &mut [u64], heap: &[HeapMeasurement]) {
    samples.sort_unstable();
    let allocations = heap.iter().map(|heap| heap.allocations).max();
    let bytes = heap.iter().map(|heap| heap.allocated_bytes).max();
    println!(
        "{}",
        serde_json::json!({
            "kind": "pick_latency", "scenario": scenario, "picks": samples.len(),
            "p50_ns": percentile(samples, 50), "p95_ns": percentile(samples, 95),
            "p99_ns": percentile(samples, 99),
            "max_heap_allocations_per_pick": allocations,
            "max_heap_bytes_per_pick": bytes,
        })
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = parse()?;
    let mut scene = Scene::from_structure(&synthetic::structure(options.residues))?;
    scene.add(rep::spacefill(sel::all()))?;
    let mut renderer = Renderer::with_profile(profile::interactive())?;
    let target = renderer.create_host_target(options.size.0, options.size.1)?;
    let camera = scene.framing_camera(
        f32::from(u16::try_from(options.size.0)?) / f32::from(u16::try_from(options.size.1)?),
    );
    let (x, y) = (options.size.0 / 2, options.size.1 / 2);
    let total = options.warmup + options.picks;
    let mut answers: Vec<StreamPick> = Vec::with_capacity(8);
    let mut streamed = Vec::with_capacity(options.picks);
    let mut streamed_heap = Vec::with_capacity(options.picks);

    let mut idle = Vec::with_capacity(options.picks);
    let mut idle_heap = Vec::with_capacity(options.picks);
    let mut in_flight = Vec::with_capacity(options.picks);
    let mut in_flight_heap = Vec::with_capacity(options.picks);
    for round in 0..total {
        let frame = renderer.render_to_host(&scene, &camera, &target)?;
        renderer.wait_host_fence_blocking(frame.fence)?;
        let started = Instant::now();
        let (heap, picked) = measure_heap(|| renderer.pick(x, y));
        picked?;
        let elapsed = u64::try_from(started.elapsed().as_nanos())?;
        if round >= options.warmup {
            idle.push(elapsed);
            idle_heap.push(heap);
        }

        renderer.render_to_host(&scene, &camera, &target)?;
        let started = Instant::now();
        let (heap, picked) = measure_heap(|| renderer.pick(x, y));
        picked?;
        let elapsed = u64::try_from(started.elapsed().as_nanos())?;
        if round >= options.warmup {
            in_flight.push(elapsed);
            in_flight_heap.push(heap);
        }

        renderer.render_to_host(&scene, &camera, &target)?;
        let started = Instant::now();
        let (heap, outcome) = measure_heap(|| -> Result<(), Box<dyn Error>> {
            renderer.request_pick(PickStream::Hover, x, y)?;
            loop {
                renderer.poll()?;
                renderer.take_picks(&scene, &mut answers);
                if !answers.is_empty() {
                    answers.clear();
                    return Ok(());
                }
            }
        });
        outcome?;
        let elapsed = u64::try_from(started.elapsed().as_nanos())?;
        if round >= options.warmup {
            streamed.push(elapsed);
            streamed_heap.push(heap);
        }
    }
    report("idle", &mut idle, &idle_heap);
    report("in_flight", &mut in_flight, &in_flight_heap);
    report("stream", &mut streamed, &streamed_heap);
    Ok(())
}
