//! The mock readback: a detached handle over a scripted buffer, plus the
//! deterministic bytes the mock queue hands back by buffer label.

use super::MockLog;
use molgfx_gpu::GpuError;
use std::sync::Arc;

/// A mock readback that owns its log, so the wait borrows nothing.
#[derive(Debug)]
pub(crate) struct MockReadback {
    pub(super) label: &'static str,
    pub(super) log: Arc<MockLog>,
}

impl molgfx_gpu::Readback for MockReadback {
    async fn resolve(&self, _offset: u64, size: u64) -> Result<Vec<u8>, GpuError> {
        mock_readback(size, self.label, &self.log)
    }
}

pub(super) fn mock_readback(
    size: u64,
    label: &'static str,
    log: &Arc<MockLog>,
) -> Result<Vec<u8>, GpuError> {
    let length = usize::try_from(size).map_err(|_| GpuError::DeviceLost)?;
    let mut bytes = vec![0; length];
    if label == "packed pick readback" {
        let row = log.pick_local_row.lock().map_or(u32::MAX, |row| *row);
        let page = log.pick_resident_page.lock().map_or(u32::MAX, |page| *page);
        let source = log
            .segment_pick_source
            .lock()
            .map_or(u32::MAX, |source| *source);
        let segment = log.segment_pick_label.lock().map_or(0, |value| *value);
        for (offset, value) in [(0, row), (256, page), (512, source), (768, segment)] {
            if let Some(word) = bytes.get_mut(offset..offset + 4) {
                word.copy_from_slice(&value.to_le_bytes());
            }
        }
    } else if label == "local row pick readback" {
        let row = log.pick_local_row.lock().map_or(u32::MAX, |row| *row);
        if let Some(word) = bytes.get_mut(..4) {
            word.copy_from_slice(&row.to_le_bytes());
        }
    } else if label == "resident page pick readback" {
        let page = log.pick_resident_page.lock().map_or(u32::MAX, |page| *page);
        if let Some(word) = bytes.get_mut(..4) {
            word.copy_from_slice(&page.to_le_bytes());
        }
    } else if label == "segment volume pick readback" {
        let source = log
            .segment_pick_source
            .lock()
            .map_or(u32::MAX, |source| *source);
        if let Some(word) = bytes.get_mut(..4) {
            word.copy_from_slice(&source.to_le_bytes());
        }
    } else if label == "segment label pick readback" {
        let label = log.segment_pick_label.lock().map_or(0, |label| *label);
        if let Some(word) = bytes.get_mut(..4) {
            word.copy_from_slice(&label.to_le_bytes());
        }
    }
    Ok(bytes)
}
