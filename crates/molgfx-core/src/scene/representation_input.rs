//! Selection inputs accepted by declarative scene authoring.

use crate::{SegmentationHandle, Select, SelectionHandle, VolumeHandle};

/// A stored selection, typed query, or compact query string.
#[derive(Clone, PartialEq, Debug)]
pub enum RepresentationInput {
    /// Reuse an already compiled selection.
    Stored(SelectionHandle),
    /// Compile one validated typed expression.
    Query(Select),
    /// Parse and cache one compact query string.
    Source(Box<str>),
    /// Draw one resident scalar grid.
    Volume(VolumeHandle),
    /// Draw one resident categorical grid.
    Segmentation(SegmentationHandle),
}

impl From<SelectionHandle> for RepresentationInput {
    fn from(value: SelectionHandle) -> Self {
        Self::Stored(value)
    }
}

impl From<Select> for RepresentationInput {
    fn from(value: Select) -> Self {
        Self::Query(value)
    }
}

impl From<&str> for RepresentationInput {
    fn from(value: &str) -> Self {
        Self::Source(value.into())
    }
}

impl From<String> for RepresentationInput {
    fn from(value: String) -> Self {
        Self::Source(value.into_boxed_str())
    }
}

impl From<VolumeHandle> for RepresentationInput {
    fn from(value: VolumeHandle) -> Self {
        Self::Volume(value)
    }
}

impl From<SegmentationHandle> for RepresentationInput {
    fn from(value: SegmentationHandle) -> Self {
        Self::Segmentation(value)
    }
}
