//! Value types for deterministic derived-resource accounting.

/// Independent hard limits for recomputable CPU and GPU data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DerivedCacheBudget {
    /// Maximum retained host bytes.
    pub cpu_bytes: u64,
    /// Maximum retained device bytes.
    pub gpu_bytes: u64,
}

impl Default for DerivedCacheBudget {
    fn default() -> Self {
        Self {
            cpu_bytes: 128 * 1024 * 1024,
            gpu_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Current and peak derived-cache charges.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct DerivedCacheUsage {
    /// Retained host bytes.
    pub cpu_bytes: u64,
    /// Retained device bytes.
    pub gpu_bytes: u64,
    /// Highest retained host charge since engine construction.
    pub peak_cpu_bytes: u64,
    /// Highest retained device charge since engine construction.
    pub peak_gpu_bytes: u64,
}

/// Eviction priority, lowest first: declaration order *is* the order the
/// ledger evicts in, so the cheapest-to-rebuild classes come first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum DerivedCacheClass {
    /// Shared visible sets: recomputed by the next cull pass, so cheapest first.
    Visibility,
    /// Packed instance records: a repack of one selection.
    RecordSet,
    /// Shared quality hierarchies, rebuilt from the packed records.
    Acceleration,
    /// Shared surface fields: rebuilt by re-running the field generation and
    /// erosion dispatches over the whole grid, which is the most expensive of
    /// the five, so they evict last among the non-timeline classes.
    SurfaceField,
    TimelineMaterialization,
}

/// Collision-free identity for one recomputable resource owner.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum DerivedCacheKey {
    #[cfg(test)]
    Test(u64),
    /// One shared packed record set, named by the key that produced it.
    RecordSet(crate::scene_gpu::RecordKey),
    /// One shared visible set.
    Visibility(crate::scene_gpu::VisibilityKey),
    /// One shared surface field.
    SurfaceField(crate::scene_gpu::SurfaceFieldKey),
    SceneAttribute(molgfx_core::AttributeHandle),
    ScenePoint(molgfx_core::PointBatchHandle),
    SceneInstance(molgfx_core::InstanceBatchHandle),
    PagedAttribute(molgfx_core::ResidencyTicket),
    PagedInstance(molgfx_core::ChunkOccurrenceId),
}

impl From<crate::scene_gpu::RecordKey> for DerivedCacheKey {
    fn from(value: crate::scene_gpu::RecordKey) -> Self {
        Self::RecordSet(value)
    }
}

impl From<crate::scene_gpu::VisibilityKey> for DerivedCacheKey {
    fn from(value: crate::scene_gpu::VisibilityKey) -> Self {
        Self::Visibility(value)
    }
}

#[cfg(test)]
impl From<u64> for DerivedCacheKey {
    fn from(value: u64) -> Self {
        Self::Test(value)
    }
}

/// Whether repeated consumers should share one derived GPU result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MaterializationPlan {
    /// Both frames are sampled by each consumer without a third allocation.
    Direct,
    /// One interpolated result is retained and shared by all consumers.
    Materialized,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct DerivedFootprint {
    pub(crate) cpu_bytes: u64,
    pub(crate) gpu_bytes: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Entry {
    pub(super) key: DerivedCacheKey,
    pub(super) class: DerivedCacheClass,
    pub(super) footprint: DerivedFootprint,
    pub(super) last_used_frame: u64,
}
