//! Shared-corpus measurements for native `MolGFX`, `Mol*` and `PyMOL` recipes.

#[path = "parity/mod.rs"]
mod parity;

#[global_allocator]
static ALLOCATOR: &stats_alloc::StatsAlloc<std::alloc::System> = &stats_alloc::INSTRUMENTED_SYSTEM;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    parity::run()
}
