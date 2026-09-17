//! Public identities, budgets and transition records for residency.

pub(super) use crate::ChunkFootprint;
use crate::{ChunkId, DatasetId};

/// Biological detail represented by one independently resident chunk.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum ResidencyDetail {
    /// Analytic atom and bond primitives.
    #[default]
    Atom,
    /// One aggregate per residue.
    Residue,
    /// Secondary-structure ribbons.
    SecondaryStructure,
    /// Domain-scale proxies.
    Domain,
}

/// Stable key for one independently resident unit.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ResidencyKey {
    /// Owning logical dataset.
    pub dataset: DatasetId,
    /// Catalog chunk.
    pub chunk: ChunkId,
    /// Biological detail represented by the chunk.
    pub detail: ResidencyDetail,
}

/// Independent hard limits for each residency tier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResidencyBudget {
    /// Maximum retained CPU payload bytes.
    pub cpu: u64,
    /// Maximum staging bytes.
    pub staging: u64,
    /// Maximum device bytes in the hot tier.
    pub gpu_hot: u64,
    /// Maximum device bytes in the warm tier.
    pub gpu_warm: u64,
    /// Maximum caller work not yet CPU-ready.
    pub in_flight: u64,
}

impl Default for ResidencyBudget {
    fn default() -> Self {
        Self {
            cpu: 512 * 1024 * 1024,
            staging: 64 * 1024 * 1024,
            gpu_hot: 512 * 1024 * 1024,
            gpu_warm: 256 * 1024 * 1024,
            in_flight: 128 * 1024 * 1024,
        }
    }
}

/// Current charged bytes per tier.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Usage {
    /// Retained CPU bytes.
    pub cpu: u64,
    /// Staging bytes.
    pub staging: u64,
    /// Hot device bytes.
    pub gpu_hot: u64,
    /// Warm device bytes.
    pub gpu_warm: u64,
    /// Work not yet CPU-ready.
    pub in_flight: u64,
}

/// Device residency tier.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ResidencyClass {
    /// Visible or imminently needed data.
    Hot,
    /// Reusable data protected only while spare capacity remains.
    Warm,
}

/// Externally visible lifecycle phase.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResidencyPhase {
    /// No resources are retained and no request is active.
    Absent,
    /// The caller is reading or decoding the chunk.
    Requested,
    /// A CPU payload is ready to upload.
    ReadyCpu,
    /// The payload is consuming staging resources.
    Uploading,
    /// Device data is available.
    Resident,
}

/// Generation-bearing token required by asynchronous completions.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ResidencyTicket {
    /// Requested chunk.
    pub key: ResidencyKey,
    generation: u64,
}

impl ResidencyTicket {
    /// Returns the generation used to reject stale work.
    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub(super) const fn issue(key: ResidencyKey, generation: u64) -> Self {
        Self { key, generation }
    }
}

/// One catalog request submitted to the machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResidencyRequest {
    /// Independently resident chunk.
    pub key: ResidencyKey,
    /// Resource charges for every phase.
    pub footprint: ChunkFootprint,
    /// Requested device tier.
    pub class: ResidencyClass,
    /// Higher values survive pressure before lower values.
    pub priority: i32,
}

/// Why active work returned to the absent phase.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FailureReason {
    /// The provider could not produce the payload.
    Provider,
    /// The payload failed validation.
    InvalidPayload,
    /// A hard resource budget cannot admit the transition.
    BudgetExceeded,
}

/// Snapshot returned without exposing internal storage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResidencySnapshot {
    /// Current lifecycle phase.
    pub phase: ResidencyPhase,
    /// Current generation, including an absent failed or cancelled generation.
    pub generation: u64,
    /// Last failure for the current generation.
    pub failure: Option<FailureReason>,
}

/// An asynchronous completion ignored because its generation is no longer active.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaleCompletion {
    /// Obsolete ticket.
    pub ticket: ResidencyTicket,
    /// Generation currently known for the key, or zero if it was never requested.
    pub current_generation: u64,
    /// Phase currently known for the key.
    pub current_phase: ResidencyPhase,
}

/// A deterministic device eviction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Eviction {
    /// Evicted chunk.
    pub key: ResidencyKey,
    /// Generation that owned the device allocation.
    pub generation: u64,
}

/// Reusable caller-owned transition outputs.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ResidencyOutput {
    /// New provider requests.
    pub requests: Vec<ResidencyTicket>,
    /// Active provider or upload work to cancel.
    pub cancellations: Vec<ResidencyTicket>,
    /// CPU-ready chunks that need a new upload after device loss.
    pub ready_uploads: Vec<ResidencyTicket>,
    /// Device allocations to release after their fence permits it.
    pub evictions: Vec<Eviction>,
    /// Completions intentionally ignored as stale.
    pub stale: Vec<StaleCompletion>,
}

impl ResidencyOutput {
    pub(super) fn clear(&mut self) {
        self.requests.clear();
        self.cancellations.clear();
        self.ready_uploads.clear();
        self.evictions.clear();
        self.stale.clear();
    }
}

/// Summary of a device-loss transition.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct DeviceLossReport {
    /// Device allocations invalidated.
    pub invalidated: usize,
    /// Chunks that can be uploaded again from retained CPU data.
    pub ready_cpu: usize,
}

/// Invalid lifecycle transition.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum ResidencyError {
    /// The ticket is current but the operation does not match its phase.
    #[error("residency transition requires {required:?}, found {actual:?}")]
    InvalidPhase {
        /// Required source phase.
        required: ResidencyPhase,
        /// Actual source phase.
        actual: ResidencyPhase,
    },
    /// The transition cannot fit within its hard resource budget.
    #[error("residency transition exceeds the {tier} budget")]
    BudgetExceeded {
        /// Resource tier that cannot admit the transition.
        tier: &'static str,
    },
    /// A key has consumed every representable asynchronous generation.
    #[error("residency ticket generation is exhausted")]
    GenerationExhausted,
}
