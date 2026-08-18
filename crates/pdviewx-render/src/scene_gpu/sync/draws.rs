//! Read-only draw and identity views over persistent scene state.

use super::GpuScene;
use crate::passes::ParticleMotionPass;
use crate::passes::SurfaceFieldPass;
use crate::scene_gpu::mesh_slot::GpuMeshSlot;
use crate::scene_gpu::slot_types::{CullDispatch, SlotShading};
use crate::scene_gpu::slots::GpuSlot;
use crate::scene_gpu::volume_slot::GpuVolumeSlot;
use pdviewx_gpu::Device;

impl<D: Device> GpuScene<D> {
    pub(crate) fn resolve_entity(
        &self,
        structure_id: u32,
        entity: pdviewx_core::EntityId,
    ) -> Option<pdviewx_core::EntityRef> {
        let structure = self.structures.get(structure_id as usize)?.handle;
        let (kind, index) = entity.unpack()?;
        Some(pdviewx_core::EntityRef {
            structure,
            kind,
            index,
        })
    }

    pub(crate) fn atom_draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.slots
            .iter()
            .filter_map(move |slot| slot.atom_draw(translucent))
    }

    pub(crate) fn bond_draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.slots
            .iter()
            .filter_map(move |slot| slot.bond_draw(translucent))
    }

    pub(crate) fn point_draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.slots
            .iter()
            .filter_map(move |slot| slot.point_draw(translucent))
    }

    pub(crate) fn cartoon_draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.slots
            .iter()
            .filter_map(move |slot| slot.cartoon_draw(translucent))
    }

    /// Caller meshes draw with the cartoon pipeline: same vertex layout, same
    /// bind group, so they simply extend that pass's draw list.
    pub(crate) fn mesh_draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.mesh_slots
            .iter()
            .filter_map(move |slot| slot.draw(translucent))
            .map(|(group, args)| (group, args, SlotShading::default()))
    }

    pub(crate) fn surface_draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.slots
            .iter()
            .filter_map(move |slot| slot.surface_draw(translucent))
    }

    pub(crate) fn quality_draws(&self) -> impl Iterator<Item = &D::BindGroup> {
        self.slots.iter().filter_map(GpuSlot::quality_draw)
    }

    pub(crate) fn volume_draws(&self) -> impl Iterator<Item = &D::BindGroup> {
        self.volume_slots.iter().filter_map(GpuVolumeSlot::draw)
    }

    pub(crate) fn segmentation_draws(&self) -> impl Iterator<Item = &D::BindGroup> {
        self.segmentation_slots
            .iter()
            .filter_map(super::super::segmentation_slot::GpuSegmentationSlot::draw)
    }

    pub(crate) fn primitive_draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        self.primitive.draw()
    }

    pub(crate) fn primitive_transparent_draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        self.primitive.transparent_draw()
    }

    pub(crate) fn cull_dispatches(&self) -> impl Iterator<Item = CullDispatch<'_, D>> {
        self.slots.iter().filter_map(GpuSlot::cull)
    }

    pub(crate) fn record_surface_fields(
        &mut self,
        encoder: &mut D::CommandEncoder,
        pass: &SurfaceFieldPass<D>,
    ) {
        for slot in &mut self.slots {
            slot.record_surface_field(encoder, pass);
        }
    }

    pub(crate) fn record_particle_motion(
        &self,
        encoder: &mut D::CommandEncoder,
        pass: &ParticleMotionPass<D>,
    ) {
        let Some((group, count)) = self.primitive.particle_motion() else {
            return;
        };
        pass.record(encoder, group, count);
    }

    pub(crate) fn has_translucency(&self) -> bool {
        self.labels.has_visible()
            || self.interactions.has_visible()
            || self.primitive.has_translucency()
            || !self.volume_slots.is_empty()
            || self.has_segmentation_translucency()
            || self.slots.iter().any(GpuSlot::is_translucent)
            || self.mesh_slots.iter().any(GpuMeshSlot::is_translucent)
    }

    pub(crate) fn interaction_draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        self.interactions.draw()
    }

    pub(crate) fn label_declutter(&self) -> Option<&D::BindGroup> {
        self.labels.declutter()
    }

    pub(crate) fn label_draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        self.labels.draw()
    }

    pub(crate) fn overlay_draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        self.overlays.draw()
    }
}
