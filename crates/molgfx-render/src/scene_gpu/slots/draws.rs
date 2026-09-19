//! Constant-cost draw routing for one persistent representation slot.

use super::{FAST_POINT_INDEX_LIMIT, GpuSlot};
use crate::passes::{SurfaceComponentPass, SurfaceFieldPass};
use crate::scene_gpu::slot_types::{CullDispatch, CullModes, SlotShading};
use molgfx_core::RepresentationKind;
use molgfx_gpu::Device;

/// Large realtime molecular streams use screen-space contact shadows. A
/// second analytic raster over more instances than this consumes the frame
/// budget while contributing mostly subpixel shadow detail.
const REALTIME_SHADOW_INSTANCES: u32 = 131_072;
impl<D: Device> GpuSlot<D> {
    pub(in crate::scene_gpu) fn record_surface_field(
        &mut self,
        encoder: &mut D::CommandEncoder,
        pass: &SurfaceFieldPass<D>,
        components: &SurfaceComponentPass<D>,
    ) {
        self.surface.record(encoder, pass, components);
    }

    pub(in crate::scene_gpu) fn atom_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, SlotShading)> {
        (self.atom_count > 0
            && (matches!(
                self.kind,
                RepresentationKind::Spacefill
                    | RepresentationKind::BallAndStick
                    | RepresentationKind::Licorice
                    | RepresentationKind::Beads
            ) || self.shading.surface_atoms())
            && self.translucent == translucent)
            .then_some((
                self.group2.as_ref()?,
                self.atom_args.as_ref()?,
                self.shading,
            ))
    }

    pub(in crate::scene_gpu) fn bond_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, SlotShading)> {
        (self.bond_count > 0 && self.translucent == translucent).then_some((
            self.group2.as_ref()?,
            self.bond_args.as_ref()?,
            self.shading,
        ))
    }

    pub(in crate::scene_gpu) fn shadow_atom_draw(
        &self,
        quality: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, SlotShading)> {
        (quality || self.atom_count <= REALTIME_SHADOW_INSTANCES).then(|| self.atom_draw(false))?
    }

    pub(in crate::scene_gpu) fn shadow_bond_draw(
        &self,
        quality: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, SlotShading)> {
        (quality || self.bond_count <= REALTIME_SHADOW_INSTANCES).then(|| self.bond_draw(false))?
    }

    pub(in crate::scene_gpu) fn point_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, SlotShading)> {
        (self.atom_count > 0
            && self.kind == RepresentationKind::Points
            && self.translucent == translucent)
            .then_some((
                self.group2.as_ref()?,
                self.atom_args.as_ref()?,
                self.shading,
            ))
    }

    pub(in crate::scene_gpu) fn cartoon_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, SlotShading)> {
        (matches!(
            self.kind,
            RepresentationKind::Cartoon
                | RepresentationKind::Trace
                | RepresentationKind::Tube
                | RepresentationKind::Rocket
                | RepresentationKind::Twister
                | RepresentationKind::PaperChain
        ) && self.translucent == translucent)
            .then(|| self.ribbon.draw())?
            .map(|(group, args)| (group, args, self.shading))
    }

    pub(in crate::scene_gpu) fn surface_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, SlotShading)> {
        (self.kind == RepresentationKind::Surface
            && self.atom_count > 0
            && !self.shading.surface_atoms()
            && self.translucent == translucent)
            .then_some((
                self.group2.as_ref()?,
                self.surface_args.as_ref()?,
                self.shading,
            ))
    }

    pub(in crate::scene_gpu) fn quality_draw(
        &self,
    ) -> Option<super::super::slot_types::QualityDraw<'_, D>> {
        if self.atom_count == 0 || self.translucent {
            return None;
        }
        match self.kind {
            RepresentationKind::Spacefill
            | RepresentationKind::BallAndStick
            | RepresentationKind::Licorice
            | RepresentationKind::Surface => Some((
                self.quality_group.as_ref()?,
                self.quality_acceleration.hardware_group(),
                self.shading,
            )),
            _ => None,
        }
    }

    pub(in crate::scene_gpu) fn record_quality_hardware(
        &mut self,
        encoder: &mut D::CommandEncoder,
    ) {
        self.quality_acceleration.record_hardware(encoder);
    }

    pub(in crate::scene_gpu) fn cull(
        &self,
        tile_groups: [u32; 2],
        fast_tile_lod: bool,
    ) -> Option<CullDispatch<'_, D>> {
        if (self.kind == RepresentationKind::Surface
            && !self.shading.surface_atoms()
            && !self.visual.has_cull_results()
            && !self.visual.has_shading_results())
            || (self.atom_count == 0 && self.bond_count == 0)
        {
            return None;
        }
        Some(CullDispatch {
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
        self.translucent
    }

    pub(in crate::scene_gpu) fn is_massive_point(&self) -> bool {
        self.kind == RepresentationKind::Points
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
