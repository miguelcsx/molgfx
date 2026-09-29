// Bond packing and indirect-draw wire records.

/// The smallest encodable bond radius, keeping the aromatic sign bit
/// unambiguous on any input.
pub(crate) const MIN_BOND_RADIUS: f32 = 1.0e-4;

/// Non-wire styling inputs used to pack one bond variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BondStyle {
    /// Quantized chemical order.
    pub order: u32,
    /// Whether the bond carries aromatic styling.
    pub aromatic: bool,
    /// Whether the bond is a metal coordination edge.
    pub metal: bool,
    /// Zero-based endpoint-offset variant for a multi-bond.
    pub variant: u32,
}

impl BondStyle {
    /// Creates one bond style without exposing GPU bit packing to callers.
    #[must_use]
    pub const fn new(order: u32, aromatic: bool, metal: bool, variant: u32) -> Self {
        Self {
            order,
            aromatic,
            metal,
            variant,
        }
    }
}

impl BondGpu {
    /// Packs a bond record.
    #[must_use]
    pub fn new(atom_a: u32, atom_b: u32, radius: f32, aromatic: bool, entity_id: EntityId) -> Self {
        Self::with_style(
            atom_a,
            atom_b,
            radius,
            BondStyle::new(1, aromatic, false, 0),
            entity_id,
        )
    }

    /// Packs a bond with chemical order and coordination flags.
    #[must_use]
    pub fn with_style(
        atom_a: u32,
        atom_b: u32,
        radius: f32,
        style: BondStyle,
        entity_id: EntityId,
    ) -> Self {
        let magnitude = radius.abs().max(MIN_BOND_RADIUS);
        let radius = if style.aromatic { -magnitude } else { magnitude };
        Self {
            atom_a,
            atom_b,
            radius,
            order: style.order.clamp(1, 3),
            flags: u32::from(style.metal) | (style.variant.min(3) << 1),
            entity_id,
        }
    }

    /// The drawn radius, always positive.
    #[must_use]
    pub fn draw_radius(self) -> f32 {
        self.radius.abs()
    }

    /// Quantized chemical order, capped at the supported analytic styles.
    #[must_use]
    pub const fn order(self) -> u32 {
        self.order
    }

    /// Whether this is a metal-coordination edge.
    #[must_use]
    pub const fn is_metal(self) -> bool {
        self.flags & 1 != 0
    }

    /// Zero-based strand index of this packed multi-bond variant.
    ///
    /// A single bond has one strand, index zero; a double bond two and a
    /// triple bond three. The shader turns the pair of this index and
    /// [`order`](Self::order) into the strand's offset and radius.
    #[must_use]
    pub const fn variant(self) -> u32 {
        (self.flags >> 1) & 3
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
