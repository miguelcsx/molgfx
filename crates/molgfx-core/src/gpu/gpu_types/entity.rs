//! Packed entity identity: the kind tag and row index every fragment writes.

use std::fmt;

/// What kind of scene entity a packed id refers to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EntityKind {
    /// An atom row.
    Atom,
    /// A bond row.
    Bond,
    /// An interaction edge.
    Edge,
    /// A text label.
    Label,
    /// A distance, angle or dihedral measurement.
    Measurement,
    /// A caller-authored ellipsoid, carbohydrate symbol or filled plane.
    Primitive,
    /// A caller-supplied indexed mesh.
    Mesh,
    /// One compact reusable-topology ligand candidate batch.
    LigandPoseBatch,
    /// A caller-authored analytic guide segment.
    Guide,
    /// A bond row from the active caller-decoded dynamic topology interval.
    DynamicBond,
    /// One row in a generic point batch.
    Point,
    /// One rigid occurrence in a shared-template instance batch.
    Instance,
    /// One analytic part in a shared instance template.
    TemplatePart,
    /// One row in a generic relation batch.
    Relation,
}

/// A scene-wide entity reference resolved from the GPU picking attachments.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EntityRef {
    /// Structure containing the entity.
    pub structure: crate::StructureHandle,
    /// Entity table kind.
    pub kind: EntityKind,
    /// Source row within that structure's table.
    pub index: u32,
}

/// A caller-owned categorical-volume segment resolved from the GPU label
/// attachments. It intentionally has no molecular structure provenance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VolumeSegmentRef {
    /// Segmented volume containing the label.
    pub volume: crate::SegmentationHandle,
    /// Exact caller-supplied voxel label.
    pub label: u32,
}

/// A pickable identity packed into 32 bits: four tag bits for the entity kind
/// and twenty-eight bits of row index. Every fragment writes one of these,
/// and picking reads it back; the packing must round-trip exactly.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityId(pub u32);

/// Failure to encode a chunk-local row in the 32-bit picking attachment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EntityIdError {
    index: u64,
}

impl EntityIdError {
    /// The rejected row index.
    #[must_use]
    pub const fn index(self) -> u64 {
        self.index
    }
}

impl fmt::Display for EntityIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "entity row {} exceeds the GPU picking limit {}",
            self.index,
            EntityId::MAX_INDEX
        )
    }
}

impl std::error::Error for EntityIdError {}

impl EntityId {
    /// The sentinel meaning "nothing here"; the clear value of the id buffer.
    pub const NONE: Self = Self(u32::MAX);

    const TAG_SHIFT: u32 = 28;
    const INDEX_MASK: u32 = (1 << Self::TAG_SHIFT) - 1;

    /// The largest packable row index. The all-ones word is the `NONE`
    /// sentinel, so the top index is reserved rather than aliasing it.
    pub const MAX_INDEX: u32 = Self::INDEX_MASK - 1;

    /// Packs a kind and chunk-local row index.
    ///
    /// Logical rows are wider than this attachment. Callers must first resolve
    /// them to a resident chunk and pass its local row; an out-of-range value
    /// is rejected instead of aliasing the last encodable entity.
    ///
    /// # Errors
    ///
    /// Returns [`EntityIdError`] when the row does not fit the picking attachment.
    pub fn pack(kind: EntityKind, index: u64) -> Result<Self, EntityIdError> {
        if index > u64::from(Self::MAX_INDEX) {
            return Err(EntityIdError { index });
        }
        let Ok(index) = u32::try_from(index) else {
            return Err(EntityIdError { index });
        };
        let tag = match kind {
            EntityKind::Atom => 0u32,
            EntityKind::Bond => 1,
            EntityKind::Edge => 2,
            EntityKind::Label => 3,
            EntityKind::Primitive => 4,
            EntityKind::Mesh => 5,
            EntityKind::LigandPoseBatch => 6,
            EntityKind::Guide => 7,
            EntityKind::DynamicBond => 8,
            EntityKind::Point => 9,
            EntityKind::Instance => 10,
            EntityKind::TemplatePart => 11,
            EntityKind::Relation => 12,
            EntityKind::Measurement => 13,
        };
        Ok(Self((tag << Self::TAG_SHIFT) | index))
    }

    /// Unpacks the kind and row index; `None` for the empty sentinel.
    #[must_use]
    pub const fn unpack(self) -> Option<(EntityKind, u32)> {
        if self.0 == u32::MAX {
            return None;
        }
        let kind = match self.0 >> Self::TAG_SHIFT {
            0 => EntityKind::Atom,
            1 => EntityKind::Bond,
            2 => EntityKind::Edge,
            3 => EntityKind::Label,
            4 => EntityKind::Primitive,
            5 => EntityKind::Mesh,
            6 => EntityKind::LigandPoseBatch,
            7 => EntityKind::Guide,
            8 => EntityKind::DynamicBond,
            9 => EntityKind::Point,
            10 => EntityKind::Instance,
            11 => EntityKind::TemplatePart,
            12 => EntityKind::Relation,
            13 => EntityKind::Measurement,
            _ => return None,
        };
        Some((kind, self.0 & Self::INDEX_MASK))
    }
}
