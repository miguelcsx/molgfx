//! Deterministic out-of-core residency with bounded resource accounting.

mod host_working_set;
mod machine;
mod machine_indices;
mod pressure;
mod types;

pub use host_working_set::{HostWorkingSet, HostWorkingSetError};
pub use machine::ResidencyMachine;
pub use types::{
    DeviceLossReport, Eviction, FailureReason, ResidencyBudget, ResidencyClass, ResidencyDetail,
    ResidencyError, ResidencyKey, ResidencyOutput, ResidencyPhase, ResidencyRequest,
    ResidencySnapshot, ResidencyTicket, StaleCompletion, Usage,
};
