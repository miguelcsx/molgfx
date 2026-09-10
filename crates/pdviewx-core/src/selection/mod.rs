//! Selection values, predicates and spatial clipping primitives.

pub(crate) mod clipping;
mod entity_selection;
pub(crate) mod select;
#[path = "selection.rs"]
pub(crate) mod values;

pub use clipping::{ClipCap, ClipPlane, ClipSet, MAX_CLIP_PLANES};
pub use entity_selection::{EntitySelection, EntitySelectionRows, GpuEntitySelection};
pub use select::{PropertyComparison, Select};
pub use values::AtomSelection;
