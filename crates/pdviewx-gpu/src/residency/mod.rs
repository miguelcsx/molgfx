//! Backend-neutral storage primitives for bounded GPU residency.
//!
//! These types own host-side bookkeeping only. Backends remain responsible
//! for allocating buffers, copying staged ranges and reporting completed
//! submission fences.

mod arena;
mod command;
mod upload;
mod upload_types;

pub use arena::{ArenaAllocation, ArenaError, ArenaMetrics, PagedArena};
pub use command::{CommandScratch, CommandScratchMetrics, ScratchFull};
pub use upload::UploadRing;
pub use upload_types::{
    FenceValue, Retirement, UploadBackpressure, UploadMetrics, UploadReservation, UploadRingConfig,
    UploadState, UploadTicket,
};
