//! GPU record layouts.
//!
//! These structs are byte-for-byte what the shaders read, so their size,
//! alignment and field order are part of the engine's binary contract; the
//! sibling tests assert every offset. Uploading a slice of them is a cast,
//! never a per-record transform.

#[cfg(test)]
#[path = "gpu_types_tests.rs"]
mod tests;

use molgfx_math::Rgba8;
use std::fmt;

/// Per-atom flag bits, packed into the atom record.
///
/// A plain `u16` newtype rather than an enum set: shaders read the same bits.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AtomFlags(pub u16);

impl AtomFlags {
    /// The atom is drawn at all.
    pub const VISIBLE: Self = Self(1);
    /// The atom belongs to the active selection.
    pub const SELECTED: Self = Self(1 << 1);
    /// The atom belongs to the current focus set.
    pub const FOCUSED: Self = Self(1 << 2);
    /// The atom is demoted context: drawn faded, still present.
    pub const GHOSTED: Self = Self(1 << 3);

    /// True when every bit of `other` is set in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of two flag sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// `self` with every bit of `other` cleared.
    #[must_use]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

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
            _ => return None,
        };
        Some((kind, self.0 & Self::INDEX_MASK))
    }
}

/// The per-atom instance record: exactly 20 bytes.
///
/// Positions are not duplicated here. The record names its source row through
/// `entity_id` and the shader gathers the coordinate from the borrowed
/// `coords[]` column, so moving an atom is one write to that column.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AtomGpu {
    /// Drawn radius, Ångström (van der Waals radius times the
    /// representation's scale).
    pub radius: f32,
    /// The element colour, not the representation's colour scheme.
    ///
    /// Colour schemes are resolved on the GPU from the packed semantic indices
    /// below, so changing a scheme never repacks or re-uploads a record. This
    /// field is what `ByElement` reads and what every other scheme falls back
    /// to when its own input row is absent.
    pub color: Rgba8,
    /// Interned element id (the atomic number); never a string.
    pub element: u16,
    /// Visibility and emphasis bits.
    pub flags: AtomFlags,
    /// Pickable identity of this atom.
    pub entity_id: EntityId,
    /// Packed semantic tag: site membership, band, edge softness, and the three
    /// palette indices the GPU colour schemes need.
    ///
    /// See [`SemanticTag`] for the bit layout.
    pub semantic: u32,
}

/// The per-bond instance record: exactly 16 bytes.
///
/// Endpoints are indices into the atom buffer, so moving an atom moves its
/// bonds with no bond re-upload. The radius carries one extra bit in its
/// sign: a negative radius marks an aromatic bond, and the magnitude is the
/// drawn radius. The magnitude is always at least `MIN_BOND_RADIUS`, so the
/// sign is never ambiguous.
#[repr(C, align(4))]
#[derive(Clone, Copy, PartialEq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BondGpu {
    /// Index of the first endpoint in the atom buffer.
    pub atom_a: u32,
    /// Index of the second endpoint in the atom buffer.
    pub atom_b: u32,
    /// Drawn radius, Ångström; sign bit set for aromatic bonds.
    pub radius: f32,
    /// Pickable identity of the source bond row.
    pub entity_id: EntityId,
}

/// One analytic interaction glyph: six aligned 16-byte lanes.
///
/// Endpoints are already in world space because they may be atom positions,
/// ring centroids or arbitrary caller-defined anchors. The renderer uploads
/// this table only when the interaction revision changes.
#[repr(C, align(16))]
#[derive(Clone, Copy, PartialEq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InteractionGpu {
    /// World-space start and pixel-stable width.
    pub start_width: [f32; 4],
    /// World-space end and repeating pattern period in pixels.
    pub end_period: [f32; 4],
    /// Linear display color as normalized RGBA.
    pub color: [f32; 4],
    /// Entity id, structure id, pattern, and directional marker flag.
    pub metadata: [u32; 4],
    /// Opacity, duty cycle, deterministic phase and arrow size in pixels.
    pub style: [f32; 4],
    /// Phase speed followed by start/end screen-space endpoint insets.
    pub animation: [f32; 4],
}

impl InteractionGpu {
    /// Packs the stable visual and picking lanes of a generic relation.
    /// Dynamic endpoint kernels overwrite only the two position lanes.
    ///
    /// # Errors
    ///
    /// Returns [`EntityIdError`] when `row` exceeds the picking attachment.
    pub fn from_relation_style(
        style: crate::RelationStyle,
        row: u64,
        pick_page: u32,
    ) -> Result<Self, EntityIdError> {
        let (period, duty) = match style.pattern {
            crate::RelationPattern::Solid => (1.0, 1.0),
            crate::RelationPattern::Dashed | crate::RelationPattern::Spring => (10.0, 0.55),
            crate::RelationPattern::Dotted => (6.0, 0.2),
        };
        let phase_byte = row.wrapping_mul(2_654_435_761).to_le_bytes()[3];
        let phase = f32::from(phase_byte) / 255.0 * period;
        Ok(Self {
            start_width: [0.0, 0.0, 0.0, style.width_pixels],
            end_period: [0.0, 0.0, 0.0, period],
            color: style.color.to_f32(),
            metadata: [
                EntityId::pack(EntityKind::Relation, row)?.0,
                pick_page,
                style.pattern as u32,
                0,
            ],
            style: [style.opacity, duty, phase, 0.0],
            animation: [
                0.0,
                style.endpoint_insets_pixels[0],
                style.endpoint_insets_pixels[1],
                f32::from(u8::from(style.depth_behind_anchors)),
            ],
        })
    }

    /// Packs one static world/world generic relation into the shared analytic
    /// glyph stream. Dynamic anchors are left for the GPU resolver.
    ///
    /// # Errors
    ///
    /// Returns [`EntityIdError`] when `row` exceeds the picking attachment.
    pub fn from_relation(
        relation: crate::Relation,
        style: crate::RelationStyle,
        row: u64,
        pick_page: u32,
    ) -> Result<Option<Self>, EntityIdError> {
        let (crate::SpatialAnchor::World(start), crate::SpatialAnchor::World(end)) =
            (relation.start, relation.end)
        else {
            return Ok(None);
        };
        let mut packed = Self::from_relation_style(style, row, pick_page)?;
        packed.start_width[..3].copy_from_slice(&start.to_array());
        packed.end_period[..3].copy_from_slice(&end.to_array());
        Ok(Some(packed))
    }

    /// Packs a caller interaction for one indirect instanced draw.
    ///
    /// # Errors
    ///
    /// Returns [`EntityIdError`] when `row` does not fit the picking attachment.
    pub fn new(
        edge: &crate::InteractionEdge,
        row: u64,
        structure_id: u32,
    ) -> Result<Self, EntityIdError> {
        let style = edge.resolved_style();
        let (start, end, directional) = match edge.direction() {
            crate::InteractionDirection::Undirected => (edge.start(), edge.end(), 0),
            crate::InteractionDirection::Forward => (edge.start(), edge.end(), 1),
            crate::InteractionDirection::Reverse => (edge.end(), edge.start(), 1),
        };
        let phase_byte = row.wrapping_mul(2_654_435_761).to_le_bytes()[3];
        let phase_unit = f32::from(phase_byte) / 255.0;
        Ok(Self {
            start_width: [
                start.position().x,
                start.position().y,
                start.position().z,
                style.width_pixels,
            ],
            end_period: [
                end.position().x,
                end.position().y,
                end.position().z,
                style.period_pixels,
            ],
            color: style.color.to_f32(),
            metadata: [
                EntityId::pack(EntityKind::Edge, row)?.0,
                structure_id,
                style.pattern as u32,
                directional,
            ],
            style: [
                style.opacity,
                style.duty_cycle,
                phase_unit * style.period_pixels,
                8.0 + style.width_pixels * 2.0,
            ],
            animation: [style.phase_speed_pixels_per_frame, 0.0, 0.0, 0.0],
        })
    }

    /// Packs a caller-authored guide into the same instanced draw.
    ///
    /// A guide carries its own style rather than deriving one from an
    /// interaction class, but it is the same analytic segment on the GPU, so it
    /// shares the glyph pass, its transparency, depth and picking.
    ///
    /// # Errors
    ///
    /// Returns [`EntityIdError`] when `row` does not fit the picking attachment.
    pub fn from_guide(
        guide: &crate::Guide,
        row: u64,
        structure_id: u32,
    ) -> Result<Self, EntityIdError> {
        let style = guide.style().sanitized();
        // A double arrow is drawn as a forward arrow whose tail also carries a
        // head; the glyph pass reads the marker flag as a count.
        let directional = match style.cap {
            crate::GuideCap::None => 0,
            crate::GuideCap::Arrow => 1,
            crate::GuideCap::DoubleArrow => 2,
        };
        let phase_byte = row.wrapping_mul(2_654_435_761).to_le_bytes()[3];
        let phase_unit = f32::from(phase_byte) / 255.0;
        Ok(Self {
            start_width: [
                guide.start().x,
                guide.start().y,
                guide.start().z,
                style.width_pixels,
            ],
            end_period: [
                guide.end().x,
                guide.end().y,
                guide.end().z,
                style.period_pixels,
            ],
            color: style.color.to_f32(),
            metadata: [
                EntityId::pack(EntityKind::Guide, row)?.0,
                structure_id,
                style.pattern as u32,
                directional,
            ],
            style: [
                style.opacity,
                style.duty_cycle,
                phase_unit * style.period_pixels,
                style.arrow_pixels,
            ],
            animation: [0.0; 4],
        })
    }
}

/// One analytic primitive instance.
///
/// The record supports an oriented carbohydrate/plane solid and a symmetric
/// inverse tensor ellipsoid without tessellation. The primitive tag selects
/// the interpretation of the shared lanes in the fragment shader.
#[repr(C, align(16))]
#[derive(Clone, Copy, PartialEq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PrimitiveGpu {
    /// World-space center and conservative bounding radius.
    pub center_radius: [f32; 4],
    /// World-space orientation quaternion, normalized.
    pub orientation: [f32; 4],
    /// Full local dimensions and final opacity.
    pub size_opacity: [f32; 4],
    /// Symmetric inverse tensor `[xx, yy, zz, xy]`.
    pub inverse_primary: [f32; 4],
    /// Remaining inverse tensor `[xz, yz]`, reserved, and motion-active flag.
    pub inverse_cross: [f32; 4],
    /// Linear display color.
    pub color: [f32; 4],
    /// Entity id, structure id, primitive shape and reserved flags.
    pub metadata: [u32; 4],
}

/// One caller-supplied visual particle-motion sample: four aligned lanes.
///
/// `velocity_step` is already multiplied by the fixed timestep and is written
/// in world space by the renderer. `metadata` is active, boundary mode, seed
/// and fixed-step respawn period. The table is indexed alongside
/// [`PrimitiveGpu`].
#[repr(C, align(16))]
#[derive(Clone, Copy, PartialEq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleMotionGpu {
    /// World-space displacement applied by one fixed simulation step.
    pub velocity_step: [f32; 4],
    /// World-space inclusive minimum bound.
    pub minimum: [f32; 4],
    /// World-space exclusive maximum bound.
    pub maximum: [f32; 4],
    /// Active flag, boundary mode, stable seed and respawn period in steps.
    pub metadata: [u32; 4],
}

include!("gpu_types_bonds.rs");
