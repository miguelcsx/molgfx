//! Stable spatial identities that do not depend on generational scene handles.

use crate::{ChunkId, ChunkOccurrenceId, DatasetId, LogicalRow};
use pdviewx_math::Vec3;

/// Spatial table addressed by one globally identified generic anchor.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ChunkSpatialKind {
    /// Provider-backed atom coordinate row.
    Atom = 0,
    /// Generic point row.
    Point = 1,
    /// Generic rigid-instance row.
    Instance = 2,
}

/// Exact logical table kind addressed outside a generational [`crate::Scene`].
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ChunkDomainKind {
    /// Provider-backed atom rows.
    Atom = 0,
    /// Generic point rows.
    Point = 1,
    /// Generic rigid-instance rows.
    Instance = 2,
    /// Flattened shared analytic-template part rows.
    TemplatePart = 3,
    /// Generic relation rows.
    Relation = 4,
}

/// Globally stable row domain used by paged attributes and visual programs.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ChunkDomainRef {
    dataset: DatasetId,
    chunk: ChunkId,
    kind: ChunkDomainKind,
}

impl ChunkDomainRef {
    /// Creates an exact scene-independent table identity.
    #[must_use]
    pub const fn new(dataset: DatasetId, chunk: ChunkId, kind: ChunkDomainKind) -> Self {
        Self {
            dataset,
            chunk,
            kind,
        }
    }

    /// Owning logical dataset.
    #[must_use]
    pub const fn dataset(self) -> DatasetId {
        self.dataset
    }

    /// Chunk owning the target rows.
    #[must_use]
    pub const fn chunk(self) -> ChunkId {
        self.chunk
    }

    /// Exact target table kind.
    #[must_use]
    pub const fn kind(self) -> ChunkDomainKind {
        self.kind
    }
}

/// One spatial row in an exact dataset chunk.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ChunkEntityRef {
    dataset: DatasetId,
    chunk: ChunkId,
    occurrence: ChunkOccurrenceId,
    row: LogicalRow,
    kind: ChunkSpatialKind,
}

impl ChunkEntityRef {
    /// Creates one globally stable spatial-row reference.
    #[must_use]
    pub const fn new(
        dataset: DatasetId,
        chunk: ChunkId,
        occurrence: ChunkOccurrenceId,
        row: LogicalRow,
        kind: ChunkSpatialKind,
    ) -> Self {
        Self {
            dataset,
            chunk,
            occurrence,
            row,
            kind,
        }
    }

    /// Dataset identity.
    #[must_use]
    pub const fn dataset(self) -> DatasetId {
        self.dataset
    }

    /// Chunk identity.
    #[must_use]
    pub const fn chunk(self) -> ChunkId {
        self.chunk
    }

    /// Exact spatial occurrence of the source chunk.
    #[must_use]
    pub const fn occurrence(self) -> ChunkOccurrenceId {
        self.occurrence
    }

    /// Full-dataset logical row.
    #[must_use]
    pub const fn row(self) -> LogicalRow {
        self.row
    }

    /// Spatial table kind.
    #[must_use]
    pub const fn kind(self) -> ChunkSpatialKind {
        self.kind
    }
}

/// One shared analytic-template part placed by an exact instance row.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TemplatePartChunkRef {
    instance: ChunkEntityRef,
    part_row: u32,
}

impl TemplatePartChunkRef {
    /// Creates an occurrence reference. Payload validation requires an
    /// instance-kind source.
    #[must_use]
    pub const fn new(instance: ChunkEntityRef, part_row: u32) -> Self {
        Self { instance, part_row }
    }

    /// Globally identified instance row.
    #[must_use]
    pub const fn instance(self) -> ChunkEntityRef {
        self.instance
    }

    /// Sphere-first template part row.
    #[must_use]
    pub const fn part_row(self) -> u32 {
        self.part_row
    }
}

/// Scene-independent endpoint resolved only after its source chunk is resident.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PagedSpatialAnchor {
    /// Immutable world-space position.
    World(Vec3),
    /// Current spatial row from another resident chunk.
    Entity(ChunkEntityRef),
    /// Local template part transformed by one resident rigid instance.
    TemplatePart(TemplatePartChunkRef),
}

impl PagedSpatialAnchor {
    pub(super) fn valid(self) -> bool {
        match self {
            Self::World(position) => position.is_finite(),
            Self::Entity(_) => true,
            Self::TemplatePart(reference) => {
                reference.instance().kind() == ChunkSpatialKind::Instance
            }
        }
    }
}

/// One logical relation row in a paged payload.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PagedRelation {
    /// First spatial endpoint.
    pub start: PagedSpatialAnchor,
    /// Second spatial endpoint.
    pub end: PagedSpatialAnchor,
}
