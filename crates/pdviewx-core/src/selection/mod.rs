//! Selection values, predicates and spatial clipping primitives.

pub(crate) mod clipping;
pub(crate) mod select;
#[path = "selection.rs"]
pub(crate) mod values;

pub use clipping::{ClipCap, ClipPlane, ClipSet, MAX_CLIP_PLANES};
pub use select::{PropertyComparison, Select};
pub use values::AtomSelection;
