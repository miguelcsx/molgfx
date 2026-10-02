//! Scene-wide `array<u32>` arena for typed columns referenced by visual programs.
//!
//! A property is uploaded once even when several representations consume it.
//! The table is rebuilt only when visual bindings change; content revisions
//! rewrite the existing column directly from the caller-owned slice without a
//! second host-side copy.

mod binding;
mod plan;
mod timeline;
mod upload;

mod arena_binding;
mod columns;
mod dispatch;
mod table;

pub(in crate::scene_gpu) use arena_binding::AttributeArenaBinding;
use columns::{
    AttributeTimelineConfig, AttributeTimelineGpu, MISSING_CHUNK, MISSING_CHUNK_LEN,
    PropertyColumn, StateColumn,
};
pub(crate) use dispatch::AttributeTimelineDispatch;
pub(in crate::scene_gpu) use table::VisualPropertyTable;
