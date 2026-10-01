//! Disjoint host-clock intervals for completed-output profiling.

/// Host elapsed time, not exclusive CPU utilization or device execution time.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct CpuStages {
    /// Scene/residency synchronization and frozen exposure preparation.
    pub preparation_ns: u64,
    /// Target preparation, uniform staging and command recording.
    pub recording_ns: u64,
    /// Bulk uniform upload and tracked queue submission.
    pub submission_ns: u64,
    /// Waiting for the tracked completion fence; includes host scheduling.
    pub completion_wait_ns: u64,
    /// Mapping and decoding timestamps; pixel export is outside this profile.
    pub timestamp_readback_ns: u64,
}
