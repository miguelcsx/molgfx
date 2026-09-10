//! Portable per-stage storage-binding validation.

use pdviewx_gpu::{BindGroupLayoutEntry, BindingType, ShaderStages};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct StageStorageCounts {
    pub(super) vertex: u32,
    pub(super) fragment: u32,
    pub(super) compute: u32,
}

pub(super) fn storage_counts(entries: &[BindGroupLayoutEntry]) -> StageStorageCounts {
    let mut counts = StageStorageCounts::default();
    for entry in entries {
        if !matches!(entry.ty, BindingType::Storage { .. }) {
            continue;
        }
        counts.vertex += u32::from(entry.visibility.contains(ShaderStages::VERTEX));
        counts.fragment += u32::from(entry.visibility.contains(ShaderStages::FRAGMENT));
        counts.compute += u32::from(entry.visibility.contains(ShaderStages::COMPUTE));
    }
    counts
}

pub(super) fn validate_storage_limit(
    label: &'static str,
    entries: &[BindGroupLayoutEntry],
    limit: u32,
) -> Result<StageStorageCounts, pdviewx_gpu::GpuError> {
    let counts = storage_counts(entries);
    let required = counts.vertex.max(counts.fragment).max(counts.compute);
    if required > limit {
        return Err(pdviewx_gpu::GpuError::LimitExceeded {
            resource: label,
            limit: u64::from(limit),
        });
    }
    Ok(counts)
}
