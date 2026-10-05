//! Typed culling modes and the borrowed resources of one compute dispatch.

use molgfx_gpu::Device;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct CullModes(u8);

impl CullModes {
    const LOD: u8 = 1 << 0;
    const FAST_POINTS: u8 = 1 << 1;
    const DIRECT_BONDS: u8 = 1 << 2;
    const CULL_VISUAL: u8 = 1 << 3;
    const SHADING_VISUAL: u8 = 1 << 4;
    const SHADING_ALL: u8 = 1 << 5;

    pub(crate) const fn with_lod(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::LOD);
        self
    }

    pub(crate) const fn with_fast_points(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::FAST_POINTS);
        self
    }

    pub(crate) const fn with_direct_bonds(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::DIRECT_BONDS);
        self
    }

    pub(crate) const fn with_cull_visual(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::CULL_VISUAL);
        self
    }

    pub(crate) const fn with_shading_visual(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::SHADING_VISUAL);
        self
    }

    pub(crate) const fn with_shading_all(mut self, enabled: bool) -> Self {
        self.0 |= Self::flag(enabled, Self::SHADING_ALL);
        self
    }

    const fn flag(enabled: bool, flag: u8) -> u8 {
        if enabled { flag } else { 0 }
    }

    pub(crate) const fn lod(self) -> bool {
        self.0 & Self::LOD != 0
    }

    pub(crate) const fn fast_points(self) -> bool {
        self.0 & Self::FAST_POINTS != 0
    }

    pub(crate) const fn direct_bonds(self) -> bool {
        self.0 & Self::DIRECT_BONDS != 0
    }

    pub(crate) const fn cull_visual(self) -> bool {
        self.0 & Self::CULL_VISUAL != 0
    }

    pub(crate) const fn shading_visual(self) -> bool {
        self.0 & Self::SHADING_VISUAL != 0
    }

    pub(crate) const fn shading_all(self) -> bool {
        self.0 & Self::SHADING_ALL != 0
    }
}

pub(crate) struct CullDispatch<'a, D: Device> {
    /// The visibility key this dispatch computes, when the slot has one.
    pub(crate) key: Option<super::visibility_cache::VisibilityKey>,
    pub(crate) atom_group: &'a D::BindGroup,
    pub(crate) bond_group: &'a D::BindGroup,
    pub(crate) visual_group: &'a D::BindGroup,
    pub(crate) atom_groups: [u32; 2],
    pub(crate) bin_groups: [u32; 2],
    pub(crate) tile_groups: [u32; 2],
    pub(crate) modes: CullModes,
    pub(crate) bond_groups: [u32; 2],
    pub(crate) visual_groups: [u32; 2],
}
