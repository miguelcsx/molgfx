//! Read-only draw and identity views over persistent scene state.

use super::GpuScene;
use crate::passes::ParticleMotionPass;
use crate::passes::{SurfaceComponentPass, SurfaceFieldPass};
use crate::scene_gpu::mesh_slot::GpuMeshSlot;
use crate::scene_gpu::slot_types::{CullDispatch, QualityDraw, SlotShading};
use crate::scene_gpu::slots::GpuSlot;
use crate::scene_gpu::volume_slot::GpuVolumeSlot;
use molgfx_gpu::Device;

impl<D: Device> GpuScene<D> {
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

    pub(crate) fn shadow_atom_draws(
        &self,
        quality: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.slots
            .iter()
            .filter_map(move |slot| slot.shadow_atom_draw(quality))
    }

    pub(crate) fn shadow_bond_draws(
        &self,
        quality: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.slots
            .iter()
            .filter_map(move |slot| slot.shadow_bond_draw(quality))
    }

    pub(crate) fn point_draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.slots
            .iter()
            .filter_map(move |slot| slot.point_draw(translucent))
    }

    pub(crate) fn generic_point_draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = (&D::BindGroup, &D::Buffer, SlotShading)> {
        self.point_batches.draws(translucent)
    }

    pub(crate) fn generic_point_dispatches(
        &self,
    ) -> impl Iterator<Item = super::super::point_batch_table::GenericPointDispatch<'_, D>> {
        self.point_batches.dispatches()
    }

    pub(crate) fn generic_point_timeline_dispatches(
        &self,
    ) -> impl Iterator<Item = super::super::point_batch_table::GenericPointTimelineDispatch<'_, D>>
    {
        self.point_batches.timeline_dispatches()
    }

    pub(crate) fn generic_instance_draws(
        &self,
        translucent: bool,
    ) -> impl Iterator<Item = super::super::instance_batch_table::GenericInstanceDraw<'_, D>> {
        self.instance_batches.draws(translucent)
    }

    pub(crate) fn generic_instance_dispatches(
        &self,
    ) -> impl Iterator<Item = super::super::instance_batch_table::GenericInstanceDispatch<'_, D>>
    {
        self.instance_batches.dispatches()
    }

    pub(crate) fn generic_instance_timeline_dispatches(
        &self,
    ) -> impl Iterator<Item = super::super::instance_batch_table::GenericInstanceTimelineDispatch<'_, D>>
    {
        self.instance_batches.timeline_dispatches()
    }

    pub(crate) fn attribute_timeline_dispatches(
        &self,
    ) -> impl Iterator<Item = super::super::visual_properties::AttributeTimelineDispatch<'_, D>>
    {
        self.visual_properties.timeline_dispatches()
    }

    pub(crate) fn relation_cull_dispatches(
        &self,
    ) -> impl Iterator<Item = super::super::interaction_table::RelationCullDispatch<'_, D>> {
        self.interactions.cull_dispatches()
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

    pub(crate) fn quality_draws(&self) -> impl Iterator<Item = QualityDraw<'_, D>> {
        self.slots.iter().filter_map(GpuSlot::quality_draw)
    }

    pub(crate) fn record_quality_hardware(
        &mut self,
        encoder: &mut D::CommandEncoder,
        enabled: bool,
    ) {
        if !enabled {
            return;
        }
        for slot in &mut self.slots {
            slot.record_quality_hardware(encoder);
        }
    }

    pub(crate) fn volume_draws(
        &self,
    ) -> impl Iterator<Item = (molgfx_core::VolumeRendering, &D::BindGroup)> {
        self.volume_slots.iter().filter_map(GpuVolumeSlot::draw)
    }

    pub(crate) fn segmentation_draws(
        &self,
    ) -> impl Iterator<Item = (super::super::SegmentationPipelineKey, &D::BindGroup)> {
        self.segmentation_slots
            .iter()
            .filter_map(super::super::segmentation_slot::GpuSegmentationSlot::draw)
    }

    /// The primitive shadow-caster table and exact opaque class ranges.
    pub(crate) fn primitive_shadow_draw(
        &self,
        quality: bool,
    ) -> Option<(&D::BindGroup, &[crate::scene_gpu::PrimitiveDrawGroup])> {
        self.primitive.shadow_draw(quality)
    }

    /// The shape-sorted primitive table with its per-class draw ranges.
    pub(crate) fn primitive_groups(
        &self,
    ) -> Option<(&D::BindGroup, &[crate::scene_gpu::PrimitiveDrawGroup])> {
        self.primitive.groups()
    }

    pub(crate) fn ligand_pose_draws(
        &self,
    ) -> Option<(
        &D::BindGroup,
        &D::Buffer,
        &[crate::scene_gpu::LigandPoseDrawGroup],
    )> {
        self.ligand_poses.draws()
    }

    pub(crate) fn ligand_pose_shadow_draws(
        &self,
        quality: bool,
    ) -> Option<(
        &D::BindGroup,
        &D::Buffer,
        &[crate::scene_gpu::LigandPoseDrawGroup],
    )> {
        self.ligand_poses.shadow_draws(quality)
    }

    pub(crate) const fn ligand_pose_statistics(&self) -> crate::scene_gpu::PoseTableStats {
        self.ligand_poses.statistics()
    }

    /// Whether any primitive group is translucent, so the transparency pass
    /// knows to run without scanning every group twice.
    pub(crate) fn has_transparent_primitives(&self) -> bool {
        self.primitive
            .groups()
            .is_some_and(|(_, groups)| groups.iter().any(|group| group.translucent))
            || self.ligand_poses.has_translucency()
            || self.instance_batches.has_translucency()
    }

    pub(crate) fn cull_dispatches(&self) -> impl Iterator<Item = CullDispatch<'_, D>> {
        let tile_groups =
            super::super::dispatch::workgroups_2d(u64::from(self.cull_tile_count).div_ceil(64));
        let fast_tile_lod = self.cull_tile_count < 262_143;
        self.slots
            .iter()
            .filter_map(move |slot| slot.cull(tile_groups, fast_tile_lod))
    }

    pub(crate) fn record_surface_fields(
        &mut self,
        encoder: &mut D::CommandEncoder,
        pass: &SurfaceFieldPass<D>,
        components: &SurfaceComponentPass<D>,
    ) {
        for slot in &mut self.slots {
            slot.record_surface_field(encoder, pass, components);
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
            || self.point_batches.has_translucency()
            || self.instance_batches.has_translucency()
            || self.primitive.has_translucency()
            || self.ligand_poses.has_translucency()
            || !self.volume_slots.is_empty()
            || self.has_segmentation_translucency()
            || self.slots.iter().any(GpuSlot::is_translucent)
            || self.mesh_slots.iter().any(GpuMeshSlot::is_translucent)
    }

    /// Dense point-only scenes have no molecular surface to integrate. Their
    /// exact ambient term is white, so the expensive cavity filters are idle.
    pub(crate) fn is_massive_points_only(&self) -> bool {
        let generic_points_are_eligible =
            !self.point_batches.has_visible() || self.point_batches.is_massive_opaque_discs();
        (self.slots.iter().any(GpuSlot::is_massive_point)
            || self.point_batches.is_massive_opaque_discs())
            && generic_points_are_eligible
            && !self.has_translucency()
            && !self.instance_batches.has_visible()
            && self.paged_spacefill_draw().is_none()
            && self.paged_bond_draw().is_none()
            && self.segmentation_slots.is_empty()
            && !self.interactions.has_visible()
            && self.atom_draws(false).next().is_none()
            && self.bond_draws(false).next().is_none()
            && self.cartoon_draws(false).next().is_none()
            && self.surface_draws(false).next().is_none()
            && self.mesh_draws(false).next().is_none()
            && self.primitive.groups().is_none()
            && self.ligand_pose_draws().is_none()
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
