//! Python adapters for declarative representation descriptors.

mod appearance;
mod kinds;
mod style;
mod target;

pub(crate) use appearance::{PyPropertyAppearance, PyPropertyAppearanceSample};
pub(crate) use kinds::{PyRepresentation, PyRepresentationKind, PyRepresentationPreset};
pub(crate) use style::PyRelationStyle;
pub(crate) use target::PyRepresentationTarget;
