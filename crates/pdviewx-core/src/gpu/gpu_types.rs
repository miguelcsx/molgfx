//! GPU record layouts.
//!
//! These structs are byte-for-byte what the shaders read, so their size,
//! alignment and field order are part of the engine's binary contract; the
//! sibling tests assert every offset. Uploading a slice of them is a cast,
//! never a per-record transform.

#[cfg(test)]
#[path = "gpu_types_tests.rs"]
mod tests;

use pdviewx_math::Rgba8;

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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// A pickable identity packed into 32 bits: three tag bits for the entity
/// kind, twenty-nine bits of row index. Every fragment writes one of these,
/// and picking reads it back; the packing must round-trip exactly.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityId(pub u32);

impl EntityId {
    /// The sentinel meaning "nothing here"; the clear value of the id buffer.
    pub const NONE: Self = Self(u32::MAX);

    const TAG_SHIFT: u32 = 29;
    const INDEX_MASK: u32 = (1 << Self::TAG_SHIFT) - 1;

    /// The largest packable row index. The all-ones word is the `NONE`
    /// sentinel, so the top index is reserved rather than aliasing it.
    pub const MAX_INDEX: u32 = Self::INDEX_MASK - 1;

    /// Packs a kind and row index. Indices are limited to `MAX_INDEX`; the
    /// largest supported structure stays well under that.
    #[must_use]
    pub const fn pack(kind: EntityKind, index: u32) -> Self {
        let tag = match kind {
            EntityKind::Atom => 0u32,
            EntityKind::Bond => 1,
            EntityKind::Edge => 2,
            EntityKind::Label => 3,
            EntityKind::Primitive => 4,
            EntityKind::Mesh => 5,
        };
        let index = if index > Self::MAX_INDEX {
            Self::MAX_INDEX
        } else {
            index
        };
        Self((tag << Self::TAG_SHIFT) | index)
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
            _ => return None,
        };
        Some((kind, self.0 & Self::INDEX_MASK))
    }
}

/// The per-atom instance record: exactly 32 bytes.
///
/// `position` is duplicated from the borrowed coordinate column only on
/// backends that cannot bind that column directly; the preferred path leaves
/// it zeroed and gathers positions in the vertex stage.
#[repr(C, align(16))]
#[derive(Clone, Copy, PartialEq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AtomGpu {
    /// World-space center, Ångström.
    pub position: [f32; 3],
    /// Drawn radius, Ångström (van der Waals radius times the
    /// representation's scale).
    pub radius: f32,
    /// Resolved display color.
    pub color: Rgba8,
    /// Interned element id (the atomic number); never a string.
    pub element: u16,
    /// Visibility and emphasis bits.
    pub flags: AtomFlags,
    /// Pickable identity of this atom.
    pub entity_id: EntityId,
    /// Packed semantic tag: site membership, secondary structure, band.
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
    /// Deterministic phase speed in pixels per frame followed by spare lanes.
    pub animation: [f32; 4],
}

impl InteractionGpu {
    /// Packs a caller interaction for one indirect instanced draw.
    #[must_use]
    pub fn new(edge: &crate::InteractionEdge, row: u32, structure_id: u32) -> Self {
        let style = edge.resolved_style();
        let (start, end, directional) = match edge.direction() {
            crate::InteractionDirection::Undirected => (edge.start(), edge.end(), 0),
            crate::InteractionDirection::Forward => (edge.start(), edge.end(), 1),
            crate::InteractionDirection::Reverse => (edge.end(), edge.start(), 1),
        };
        let phase_byte = u8::try_from((row.wrapping_mul(2_654_435_761) >> 24) & 0xff)
            .map_or(u8::MAX, |value| value);
        let phase_unit = f32::from(phase_byte) / 255.0;
        Self {
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
                EntityId::pack(EntityKind::Edge, row).0,
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
        }
    }

    /// Packs a caller-authored guide into the same instanced draw.
    ///
    /// A guide carries its own style rather than deriving one from an
    /// interaction class, but it is the same analytic segment on the GPU, so it
    /// shares the glyph pass, its transparency, depth and picking.
    #[must_use]
    pub fn from_guide(guide: &crate::Guide, row: u32, structure_id: u32) -> Self {
        let style = guide.style().sanitized();
        // A double arrow is drawn as a forward arrow whose tail also carries a
        // head; the glyph pass reads the marker flag as a count.
        let directional = match style.cap {
            crate::GuideCap::None => 0,
            crate::GuideCap::Arrow => 1,
            crate::GuideCap::DoubleArrow => 2,
        };
        let phase_byte = u8::try_from((row.wrapping_mul(2_654_435_761) >> 24) & 0xff)
            .map_or(u8::MAX, |value| value);
        let phase_unit = f32::from(phase_byte) / 255.0;
        Self {
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
                EntityId::pack(EntityKind::Edge, row).0,
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
        }
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
    /// Remaining inverse tensor `[xz, yz]`, plus two spare lanes.
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

/// The smallest encodable bond radius, keeping the aromatic sign bit
/// unambiguous on any input.
pub const MIN_BOND_RADIUS: f32 = 1.0e-4;

impl BondGpu {
    /// Packs a bond record.
    #[must_use]
    pub fn new(atom_a: u32, atom_b: u32, radius: f32, aromatic: bool, entity_id: EntityId) -> Self {
        let magnitude = radius.abs().max(MIN_BOND_RADIUS);
        let radius = if aromatic { -magnitude } else { magnitude };
        Self {
            atom_a,
            atom_b,
            radius,
            entity_id,
        }
    }

    /// The drawn radius, always positive.
    #[must_use]
    pub fn draw_radius(self) -> f32 {
        self.radius.abs()
    }

    /// Whether the aromatic bit is set.
    #[must_use]
    pub fn is_aromatic(self) -> bool {
        self.radius.is_sign_negative()
    }
}

/// The non-indexed indirect draw arguments the cull pass writes: exactly the
/// wire format the GPU consumes, 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawIndirectArgs {
    /// Vertices per instance; 6 (two triangles) for an impostor quad.
    pub vertex_count: u32,
    /// Instances to draw; written by the cull pass, never read by the CPU.
    pub instance_count: u32,
    /// First vertex.
    pub first_vertex: u32,
    /// First instance; kept 0, the slot base rides in a uniform instead.
    pub first_instance: u32,
}
