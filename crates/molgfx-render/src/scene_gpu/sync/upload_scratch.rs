//! Bounded lifetime for large CPU-to-GPU packing arenas.

use super::GpuScene;
use molgfx_gpu::Device;

const RETAINED_BYTES: usize = 8 * 1024 * 1024;

impl<D: Device> GpuScene<D> {
    pub(super) fn release_upload_scratch(&mut self) {
        release(&mut self.atom_scratch);
        release(&mut self.bond_scratch);
        release(&mut self.compaction_scratch);
    }
}

fn release<T>(values: &mut Vec<T>) {
    let bytes = values.capacity().saturating_mul(std::mem::size_of::<T>());
    if bytes > RETAINED_BYTES {
        *values = Vec::new();
    } else {
        values.clear();
    }
}

#[cfg(test)]
#[path = "upload_scratch_tests.rs"]
mod tests;
