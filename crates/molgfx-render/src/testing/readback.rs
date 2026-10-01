//! Detached mock readback with bounded caller-owned scratch and scripted bytes.

use super::MockLog;
use molgfx_gpu::{BufferUsage, GpuError};
use std::future::Future;
use std::sync::Arc;

/// A mock readback that owns its log, so the wait borrows no renderer state.
#[derive(Debug)]
pub(crate) struct MockReadback {
    pub(super) label: &'static str,
    pub(super) size: u64,
    pub(super) usage: BufferUsage,
    pub(super) log: Arc<MockLog>,
}

impl molgfx_gpu::Readback for MockReadback {
    fn resolve(
        &self,
        offset: u64,
        size: u64,
    ) -> impl Future<Output = Result<Vec<u8>, GpuError>> + '_ {
        std::future::ready(self.resolve_bytes(offset, size))
    }

    fn resolve_into<'a>(
        &'a self,
        offset: u64,
        size: u64,
        output: &'a mut [u8],
    ) -> impl Future<Output = Result<(), GpuError>> + 'a {
        std::future::ready(self.copy_bytes(offset, size, output))
    }
}

impl MockReadback {
    pub(super) fn resolve_bytes(&self, offset: u64, size: u64) -> Result<Vec<u8>, GpuError> {
        let length = self.validate_range(offset, size, None)?;
        let mut output = vec![0; length];
        self.fill_bytes(offset, &mut output);
        Ok(output)
    }

    pub(super) fn copy_bytes(
        &self,
        offset: u64,
        size: u64,
        output: &mut [u8],
    ) -> Result<(), GpuError> {
        let length = self.validate_range(offset, size, Some(output.len()))?;
        self.fill_bytes(offset, &mut output[..length]);
        Ok(())
    }

    fn validate_range(
        &self,
        offset: u64,
        size: u64,
        capacity: Option<usize>,
    ) -> Result<usize, GpuError> {
        let end = offset.checked_add(size).ok_or_else(|| GpuError::Runtime {
            detail: "GPU readback range overflows its address space".to_owned(),
        })?;
        if end > self.size || size == 0 || !offset.is_multiple_of(8) || !size.is_multiple_of(4) {
            return Err(GpuError::Runtime {
                detail: "GPU readback range is empty, unaligned, or exceeds its buffer".to_owned(),
            });
        }
        let length = usize::try_from(size).map_err(|_| GpuError::Runtime {
            detail: "GPU readback range exceeds the host address space".to_owned(),
        })?;
        if capacity.is_some_and(|capacity| capacity < length) {
            return Err(GpuError::Runtime {
                detail: "GPU readback output is shorter than the requested range".to_owned(),
            });
        }
        if !self.usage.contains(BufferUsage::MAP_READ) {
            return Err(GpuError::Runtime {
                detail: "GPU readback buffer does not permit host reads".to_owned(),
            });
        }
        Ok(length)
    }

    fn fill_bytes(&self, offset: u64, output: &mut [u8]) {
        output.fill(0);
        let read = |value: &std::sync::Mutex<u32>, fallback| value.lock().map_or(fallback, |v| *v);
        match self.label {
            "packed pick readback" => {
                for (position, value) in [
                    (0, read(&self.log.pick_local_row, u32::MAX)),
                    (256, read(&self.log.pick_resident_page, u32::MAX)),
                    (512, read(&self.log.segment_pick_source, u32::MAX)),
                    (768, read(&self.log.segment_pick_label, 0)),
                ] {
                    write_word(output, offset, position, value);
                }
            }
            "local row pick readback" => {
                write_word(output, offset, 0, read(&self.log.pick_local_row, u32::MAX));
            }
            "resident page pick readback" => {
                write_word(
                    output,
                    offset,
                    0,
                    read(&self.log.pick_resident_page, u32::MAX),
                );
            }
            "segment volume pick readback" => {
                write_word(
                    output,
                    offset,
                    0,
                    read(&self.log.segment_pick_source, u32::MAX),
                );
            }
            "segment label pick readback" => {
                write_word(output, offset, 0, read(&self.log.segment_pick_label, 0));
            }
            _ => {}
        }
    }
}

fn write_word(output: &mut [u8], offset: u64, position: u64, value: u32) {
    let Some(relative) = position
        .checked_sub(offset)
        .and_then(|p| usize::try_from(p).ok())
    else {
        return;
    };
    if let Some(word) = output.get_mut(relative..relative.saturating_add(4)) {
        word.copy_from_slice(&value.to_le_bytes());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "readback_tests.rs"]
mod tests;
