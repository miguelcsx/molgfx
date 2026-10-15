//! Shared-corpus measurements for native `MolGFX`, `Mol*` and `PyMOL` recipes.
//! One manifest and one summarizer across independently identified recipes.

mod config;
mod external;
mod golden;
mod metadata;
mod native;
mod runner;
mod script;
mod sheet;
mod telemetry;

#[global_allocator]
static ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> = &stats_alloc::INSTRUMENTED_SYSTEM;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    runner::run()
}
