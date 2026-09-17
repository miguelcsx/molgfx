// Bond packing and indirect-draw wire records.

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
