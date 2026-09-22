//! Resident-memory and allocation measurements for representative scenes.
//!
//! This is process-isolated instrumentation, not a benchmark: it reports what a
//! scene costs in live bytes and how many allocations it took to build, which
//! criterion cannot see. The counters are the instrumented system allocator's,
//! so every figure is observed rather than estimated, and each case prints one
//! machine-readable line so two revisions can be compared without parsing
//! prose.
//!
//! Live bytes is the figure that matters for a renderer holding a scene; the
//! difference between what a construction requests in total and what it still
//! holds is the transient cost a caller pays to get there.
//!
//! ```text
//! cargo run -p molgfx-bench --release --bin resources -- <case>
//! ```

// The instrumented allocator is process-wide, so it can only be installed by a
// binary: a library that declared it would impose the counters on every crate
// that ever linked it.
#[global_allocator]
static ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> = &stats_alloc::INSTRUMENTED_SYSTEM;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let selected = match arguments.first() {
        Some(case) => case.as_str(),
        None => "all",
    };
    for record in molgfx_bench::resources::run(selected) {
        println!("{}", record.line());
    }
}
