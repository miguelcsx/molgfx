//! Generational handles and the slot map behind them.
//!
//! A handle stays valid across unrelated edits and detects its own staleness:
//! removing an entry bumps the slot's generation, so a handle held from
//! before the removal no longer resolves. All operations are `O(1)`.

pub(crate) use super::slot_map::SlotMap;
use serde::{Deserialize, Serialize};

/// Identifies a placed structure within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct StructureHandle(pub(crate) RawHandle);

/// Identifies a representation within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RepresentationHandle(pub(crate) RawHandle);

/// Identifies a stored selection within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SelectionHandle(pub(crate) RawHandle);

/// Identifies a caller-supplied scalar volume within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct VolumeHandle(pub(crate) RawHandle);

/// Identifies a caller-supplied categorical label volume within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SegmentationHandle(pub(crate) RawHandle);

/// Identifies a weighted caller-declared ensemble within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct EnsembleHandle(pub(crate) RawHandle);

/// Identifies a caller-supplied interaction edge within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct InteractionHandle(pub(crate) RawHandle);

/// Identifies a caller-authored guide within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct GuideHandle(pub(crate) RawHandle);

/// Identifies a caller-supplied mesh within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MeshHandle(pub(crate) RawHandle);

/// Identifies one transform-only occurrence of a shared mesh.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct MeshInstanceHandle(pub(crate) RawHandle);

/// Identifies one depth-independent screen overlay.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct OverlayHandle(pub(crate) RawHandle);

/// Identifies a persistent annotation within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct AnnotationHandle(pub(crate) RawHandle);

/// Identifies a persistent measurement within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct MeasurementHandle(pub(crate) RawHandle);

/// Identifies one immutable caller-supplied typed attribute column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct AttributeHandle(pub(crate) RawHandle);

/// Transitional handle for the pre-schema-8 scalar-property table.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct AtomPropertyHandle(pub(crate) RawHandle);

/// Identifies one caller-supplied analytic primitive.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PrimitiveHandle(pub(crate) RawHandle);

/// Identifies one shared-layout point batch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PointBatchHandle(pub(crate) RawHandle);

/// Identifies one compact batch of rigid instances sharing an analytic template.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct InstanceBatchHandle(pub(crate) RawHandle);

/// Identifies one caller-supplied batch of generic spatial relations.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RelationBatchHandle(pub(crate) RawHandle);

/// Transitional handle for the pre-schema-8 pose table.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct LigandPoseBatchHandle(pub(crate) RawHandle);

/// Identifies one independently warped timeline track.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct TimelineTrackHandle(pub(crate) RawHandle);

macro_rules! handle_identity {
    ($($handle:ident),+ $(,)?) => {
        $(
            impl $handle {
                /// Stable slot index used by scene manifests and GPU tables.
                pub const fn row(self) -> u32 {
                    self.0.index
                }

                /// Generation component used to reject stale links.
                pub const fn generation(self) -> u32 {
                    self.0.generation
                }
            }
        )+
    };
}

handle_identity!(
    StructureHandle,
    RepresentationHandle,
    SelectionHandle,
    VolumeHandle,
    SegmentationHandle,
    EnsembleHandle,
    AttributeHandle,
    AtomPropertyHandle,
    PrimitiveHandle,
    PointBatchHandle,
    InstanceBatchHandle,
    RelationBatchHandle,
    LigandPoseBatchHandle,
    TimelineTrackHandle,
    MeshHandle,
    MeshInstanceHandle,
    OverlayHandle,
);

impl InteractionHandle {
    pub(crate) const fn row(self) -> u32 {
        self.0.row()
    }
}

impl GuideHandle {
    pub(crate) const fn row(self) -> u32 {
        self.0.row()
    }
}

impl AnnotationHandle {
    /// Stable storage slot the handle occupies.
    ///
    /// The renderer packs this slot into a pick token, so resolution matches a
    /// pick against the handle whose row equals it.
    #[must_use]
    pub const fn row(self) -> u32 {
        self.0.row()
    }
}

impl MeasurementHandle {
    /// Stable storage slot the handle occupies.
    ///
    /// The renderer packs this slot into a pick token, so resolution matches a
    /// pick against the handle whose row equals it.
    #[must_use]
    pub const fn row(self) -> u32 {
        self.0.row()
    }
}

/// Slot index plus generation; the unit every typed handle wraps.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub(crate) struct RawHandle {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl RawHandle {
    pub(crate) const fn from_parts(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub(crate) const fn row(self) -> u32 {
        self.index
    }

    pub(crate) const fn generation(self) -> u32 {
        self.generation
    }
}

#[cfg(test)]
impl RawHandle {
    pub(crate) const fn new_for_test(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
}
