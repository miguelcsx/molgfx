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
}

/// A pickable identity packed into 32 bits: two tag bits for the entity
/// kind, thirty bits of row index. Every fragment writes one of these, and
/// picking reads it back; the packing must round-trip exactly.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityId(pub u32);

impl EntityId {
    /// The sentinel meaning "nothing here"; the clear value of the id buffer.
    pub const NONE: Self = Self(u32::MAX);

    const TAG_SHIFT: u32 = 30;
    const INDEX_MASK: u32 = (1 << Self::TAG_SHIFT) - 1;

    /// The largest packable row index. One label slot short of 2^30: the
    /// all-ones word is the `NONE` sentinel, so the top label index is
    /// reserved rather than aliasing it.
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
            _ => EntityKind::Label,
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

/// The per-bond instance record: exactly 12 bytes.
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
}

/// The smallest encodable bond radius, keeping the aromatic sign bit
/// unambiguous on any input.
pub const MIN_BOND_RADIUS: f32 = 1.0e-4;

impl BondGpu {
    /// Packs a bond record.
    #[must_use]
    pub fn new(atom_a: u32, atom_b: u32, radius: f32, aromatic: bool) -> Self {
        let magnitude = radius.abs().max(MIN_BOND_RADIUS);
        let radius = if aromatic { -magnitude } else { magnitude };
        Self {
            atom_a,
            atom_b,
            radius,
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
