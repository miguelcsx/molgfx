//! Constant-cost draw routing for one persistent representation slot.

use super::{FAST_POINT_INDEX_LIMIT, GpuSlot};
use crate::engine::pipeline_cache::SpecializationKey;
use crate::scene_gpu::slot_types::{
    CullDispatch, CullModes, DrawFamily, DrawSpecializations, SlotShading,
};
use molgfx_core::RepresentationKind;
use molgfx_gpu::Device;

/// Large realtime molecular streams use screen-space contact shadows. A
/// second analytic raster over more instances than this consumes the frame
/// budget while contributing mostly subpixel shadow detail.
const REALTIME_SHADOW_INSTANCES: u32 = 131_072;
impl<D: Device> GpuSlot<D> {
    /// The key this slot's named family settled under, when it has one.
    ///
    /// The settle pass writes the table once per frame, before any pass
    /// records, so a reader either finds a settled key or finds none.
    pub(in crate::scene_gpu) const fn specialization(
        &self,
        family: DrawFamily,
    ) -> Option<SpecializationKey> {
        self.specialized[family.index()]
    }

    /// The keys this slot resolved this frame, in family order.
    pub(in crate::scene_gpu) const fn keys(&self) -> &DrawSpecializations {
        &self.specialized
    }

    /// The keys this slot resolved this frame, for the settle pass to write.
    pub(in crate::scene_gpu) fn keys_mut(&mut self) -> &mut DrawSpecializations {
        &mut self.specialized
    }

    /// Whether this slot's style draws through generated code at all.
    ///
    /// Only a style with fragment-stage instructions has a sibling unit, so a
    /// slot without one never resolves a family and every pass draws it
    /// through the interpreter.
    pub(in crate::scene_gpu) const fn has_fragment_style(&self) -> bool {
        self.shading.fragment_visual()
    }

    /// The family the sphere impostor pass draws this slot through.
    pub(in crate::scene_gpu) const fn sphere_family(&self) -> DrawFamily {
        if self.shading.clipped() {
            DrawFamily::SphereClipped
        } else {
            DrawFamily::Sphere
        }
    }

    /// The family the bond pass draws this slot through.
    pub(in crate::scene_gpu) const fn bond_family(&self) -> DrawFamily {
        if self.shading.wire() {
            DrawFamily::BondWire
        } else {
            DrawFamily::Bond
        }
    }

    /// The family the surface pass draws this slot through.
    pub(in crate::scene_gpu) const fn surface_family(&self) -> DrawFamily {
        if self.shading.surface_grid() {
            DrawFamily::SurfaceGrid
        } else {
            DrawFamily::SurfaceUnion
        }
    }

    /// Every family this slot draws through this frame.
    ///
    /// A family is listed only when the slot would actually route a draw to
    /// it, so a spacefill slot never compiles a surface or occlusion pipeline.
    /// Families the frame never draws are left unsettled, and their draws
    /// carry no pipeline.
    pub(in crate::scene_gpu) fn drawn_families(&self) -> impl Iterator<Item = DrawFamily> {
        let mut families = [None; super::super::slot_types::DRAW_FAMILIES];
        let mut add = |family: DrawFamily| {
            families[family.index()] = Some(family);
        };
        if self.draws_atoms(false) {
            add(self.sphere_family());
            add(DrawFamily::ShadowSphere);
        }
        if self.draws_bonds(false) {
            add(self.bond_family());
            add(DrawFamily::ShadowBond);
        }
        if self.point_draw(false).is_some() {
            add(DrawFamily::Point);
        }
        if self.cartoon_draw(false).is_some() {
            add(DrawFamily::Cartoon);
            add(DrawFamily::ShadowRibbon);
        }
        if self.surface_draw(false).is_some() {
            add(self.surface_family());
        }
        if matches!(
            self.kind,
            RepresentationKind::Spacefill
                | RepresentationKind::BallAndStick
                | RepresentationKind::Licorice
                | RepresentationKind::Surface
        ) && self.visible
            && self.atom_count > 0
            && !self.translucent
        {
            add(DrawFamily::AmbientOcclusion);
        }
        families.into_iter().flatten()
    }

    /// Whether this slot would draw atoms, ignoring whether the arena exists.
    pub(in crate::scene_gpu) fn draws_atoms(&self, translucent: bool) -> bool {
        self.visible
            && self.atom_count > 0
            && (matches!(
                self.kind,
                RepresentationKind::Spacefill
                    | RepresentationKind::BallAndStick
                    | RepresentationKind::Licorice
                    | RepresentationKind::Beads
            ) || self.shading.surface_atoms())
            && self.translucent == translucent
    }

    /// Whether this slot would draw bonds, ignoring whether the arena exists.
    pub(in crate::scene_gpu) fn draws_bonds(&self, translucent: bool) -> bool {
        self.visible && self.bond_count > 0 && self.translucent == translucent
    }

    pub(in crate::scene_gpu) fn atom_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, u64, SlotShading)> {
        (self.visible
            && self.atom_count > 0
            && (matches!(
                self.kind,
                RepresentationKind::Spacefill
                    | RepresentationKind::BallAndStick
                    | RepresentationKind::Licorice
                    | RepresentationKind::Beads
            ) || self.shading.surface_atoms())
            && self.translucent == translucent)
            .then_some((self.group2.as_ref()?, self.atom_args?, self.shading))
    }

    pub(in crate::scene_gpu) fn bond_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, u64, SlotShading)> {
        (self.visible && self.bond_count > 0 && self.translucent == translucent).then_some((
            self.group2.as_ref()?,
            self.bond_args?,
            self.shading,
        ))
    }

    pub(in crate::scene_gpu) fn shadow_atom_draw(
        &self,
        quality: bool,
    ) -> Option<(&D::BindGroup, u64, SlotShading)> {
        (quality || self.atom_count <= REALTIME_SHADOW_INSTANCES).then(|| self.atom_draw(false))?
    }

    pub(in crate::scene_gpu) fn shadow_bond_draw(
        &self,
        quality: bool,
    ) -> Option<(&D::BindGroup, u64, SlotShading)> {
        (quality || self.bond_count <= REALTIME_SHADOW_INSTANCES).then(|| self.bond_draw(false))?
    }

    pub(in crate::scene_gpu) fn point_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, u64, SlotShading)> {
        (self.visible
            && self.atom_count > 0
            && self.kind == RepresentationKind::Points
            && self.translucent == translucent)
            .then_some((self.group2.as_ref()?, self.atom_args?, self.shading))
    }

    pub(in crate::scene_gpu) fn cartoon_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, SlotShading)> {
        (self.visible
            && matches!(
                self.kind,
                RepresentationKind::Cartoon
                    | RepresentationKind::Trace
                    | RepresentationKind::Tube
                    | RepresentationKind::Rocket
                    | RepresentationKind::Twister
                    | RepresentationKind::PaperChain
            )
            && self.translucent == translucent)
            .then(|| self.ribbon.draw())?
            .map(|(group, args)| (group, args, self.shading))
    }

    pub(in crate::scene_gpu) fn surface_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, u64, SlotShading)> {
        (self.visible
            && self.kind == RepresentationKind::Surface
            && self.atom_count > 0
            && !self.shading.surface_atoms()
            && self.translucent == translucent)
            .then_some((self.group2.as_ref()?, self.surface_args?, self.shading))
    }

    pub(in crate::scene_gpu) fn quality_draw(
        &self,
    ) -> Option<super::super::slot_types::QualityDraw<'_, D>> {
        if !self.visible || self.atom_count == 0 || self.translucent {
            return None;
        }
        match self.kind {
            RepresentationKind::Spacefill
            | RepresentationKind::BallAndStick
            | RepresentationKind::Licorice
            | RepresentationKind::Surface => Some((
                self.quality_group.as_ref()?,
                self.hardware.group(),
                self.shading,
                None,
            )),
            _ => None,
        }
    }

    pub(in crate::scene_gpu) fn record_quality_hardware(
        &mut self,
        encoder: &mut D::CommandEncoder,
    ) {
        if self.visible {
            self.hardware.record(encoder);
        }
    }

    pub(in crate::scene_gpu) fn cull(
        &self,
        tile_groups: [u32; 2],
        fast_tile_lod: bool,
    ) -> Option<CullDispatch<'_, D>> {
        if !self.visible
            || (self.kind == RepresentationKind::Surface
                && !self.shading.surface_atoms()
                && !self.visual.has_cull_results()
                && !self.visual.has_shading_results())
            || (self.atom_count == 0 && self.bond_count == 0)
        {
            return None;
        }
        Some(CullDispatch {
            key: self.visibility_key,
            atom_group: self.atom_cull_group.as_ref()?,
            bond_group: self.bond_cull_group.as_ref()?,
            visual_group: self.visual_cull_group.as_ref()?,
            atom_groups: if self.kind == RepresentationKind::Lines || is_spline_kind(self.kind) {
                [0, 0]
            } else {
                super::super::dispatch::workgroups_2d(u64::from(self.atom_count).div_ceil(64))
            },
            bin_groups: super::super::dispatch::workgroups_2d(
                u64::from(
                    self.atom_count
                        .div_ceil(self.atom_count.div_ceil(65_536).max(1)),
                )
                .div_ceil(64),
            ),
            tile_groups,
            modes: CullModes::default()
                .with_lod(self.atom_count >= REALTIME_SHADOW_INSTANCES)
                .with_fast_points(
                    self.kind == RepresentationKind::Points
                        && self.bond_count == 0
                        && self.atom_count >= REALTIME_SHADOW_INSTANCES
                        && self.atom_count < FAST_POINT_INDEX_LIMIT
                        && fast_tile_lod,
                )
                .with_direct_bonds(self.kind == RepresentationKind::Lines)
                .with_cull_visual(self.visual.has_cull_results())
                .with_shading_visual(self.visual.has_shading_results())
                .with_shading_all(
                    self.kind == RepresentationKind::Lines
                        || is_spline_kind(self.kind)
                        || (self.kind == RepresentationKind::Surface
                            && !self.shading.surface_atoms()),
                ),
            bond_groups: super::super::dispatch::workgroups_2d(
                u64::from(self.bond_count).div_ceil(64),
            ),
            visual_groups: super::super::dispatch::workgroups_2d(
                match u64::try_from(self.visual.entity_count()) {
                    Ok(count) => count.div_ceil(64),
                    Err(_) => u64::MAX.div_ceil(64),
                },
            ),
        })
    }

    pub(in crate::scene_gpu) const fn is_translucent(&self) -> bool {
        self.visible && self.translucent
    }

    pub(in crate::scene_gpu) fn is_massive_point(&self) -> bool {
        self.visible
            && self.kind == RepresentationKind::Points
            && self.atom_count >= REALTIME_SHADOW_INSTANCES
            && !self.translucent
    }
}

const fn is_spline_kind(kind: RepresentationKind) -> bool {
    matches!(
        kind,
        RepresentationKind::Cartoon
            | RepresentationKind::Trace
            | RepresentationKind::Tube
            | RepresentationKind::Rocket
            | RepresentationKind::Twister
            | RepresentationKind::PaperChain
    )
}
