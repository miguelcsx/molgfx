//! Constant-cost draw routing for one persistent representation slot.

use super::GpuSlot;
use crate::passes::SurfaceFieldPass;
use crate::scene_gpu::slot_types::{CullDispatch, SlotShading};
use pdviewx_core::RepresentationKind;
use pdviewx_gpu::Device;

impl<D: Device> GpuSlot<D> {
    pub(in crate::scene_gpu) fn record_surface_field(
        &mut self,
        encoder: &mut D::CommandEncoder,
        pass: &SurfaceFieldPass<D>,
    ) {
        self.surface.record(encoder, pass);
    }

    pub(in crate::scene_gpu) fn atom_draw(
        &self,
        translucent: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, SlotShading)> {
        (self.atom_count > 0
            && matches!(
                self.kind,
                RepresentationKind::Spacefill
                    | RepresentationKind::BallAndStick
                    | RepresentationKind::Licorice
                    | RepresentationKind::Beads
            )
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
            && self.translucent == translucent)
            .then_some((
                self.group2.as_ref()?,
                self.surface_args.as_ref()?,
                self.shading,
            ))
    }

    pub(in crate::scene_gpu) fn quality_draw(&self) -> Option<&D::BindGroup> {
        if self.atom_count == 0 || self.translucent {
            return None;
        }
        match self.kind {
            RepresentationKind::Spacefill
            | RepresentationKind::BallAndStick
            | RepresentationKind::Licorice
            | RepresentationKind::Surface => self.group2.as_ref(),
            _ => None,
        }
    }

    pub(in crate::scene_gpu) fn cull(&self) -> Option<CullDispatch<'_, D>> {
        if self.kind == RepresentationKind::Surface
            || (self.atom_count == 0 && self.bond_count == 0)
        {
            return None;
        }
        Some(CullDispatch {
            group: self.cull_group.as_ref()?,
            atom_groups: if self.kind == RepresentationKind::Lines {
                0
            } else {
                self.atom_count.div_ceil(64)
            },
            bond_groups: self.bond_count.div_ceil(64),
        })
    }

    pub(in crate::scene_gpu) const fn is_translucent(&self) -> bool {
        self.translucent
    }
}
