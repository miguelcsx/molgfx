//! Allocation-free counters for compact ligand pose synchronization.

use super::Engine;
use crate::{DerivedCacheUsage, ResidencyCounters, ResidencyMetrics};
use molgfx_gpu::Device;

/// Counts from the most recent successful scene synchronization.
///
/// Pose counts describe candidates. Instance counts describe the analytic
/// atom and bond impostors produced after reusable topology expansion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LigandPoseStats {
    /// Candidates stored across all scene pose batches, including hidden and
    /// zero-alpha candidates.
    pub resident_poses: u64,
    /// Alpha-positive candidates in visible batches whose owner is resident.
    pub visible_poses: u64,
    /// Candidates retained by the active realtime or cinematic workload policy.
    pub selected_poses: u64,
    /// Topology-expanded analytic instances submitted to beauty passes.
    pub beauty_instances: u64,
    /// Opaque analytic instances repeated by the enabled shadow route.
    pub shadow_instances: u64,
    /// Indirect draw groups across beauty and enabled shadow routes.
    pub draw_groups: u32,
}

impl<D: Device> Engine<D> {
    /// Returns current and peak recomputable cache charges in constant time.
    #[must_use]
    pub const fn derived_cache_usage(&self) -> DerivedCacheUsage {
        self.derived_cache.usage()
    }

    /// Returns real workspace and lifecycle counters without scanning scene data.
    #[must_use]
    pub fn residency_metrics(&self) -> ResidencyMetrics {
        self.scene_gpu.residency_metrics()
    }

    /// Returns flat cumulative counters plus the current resident-byte gauge.
    #[must_use]
    pub fn residency_counters(&self) -> ResidencyCounters {
        self.residency_metrics().counters()
    }

    /// Returns compact ligand workload counters without scanning the scene.
    #[must_use]
    pub fn ligand_pose_stats(&self) -> LigandPoseStats {
        let stats = self.scene_gpu.ligand_pose_statistics();
        LigandPoseStats {
            resident_poses: stats.resident_poses,
            visible_poses: stats.visible_poses,
            selected_poses: stats.selected_poses,
            beauty_instances: stats.beauty_instances,
            shadow_instances: stats.shadow_instances,
            draw_groups: stats.draw_groups,
        }
    }
}
