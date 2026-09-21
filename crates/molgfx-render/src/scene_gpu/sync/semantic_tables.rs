//! Synchronization for non-molecular semantic draw tables.

use super::GpuScene;
use crate::error::RenderError;
use crate::scene_gpu::generic_visual::GenericVisualResources;
use crate::scene_gpu::instance_batch_table::InstanceBatchSync;
use crate::scene_gpu::interaction_table::{RelationSources, RelationSync};
use crate::scene_gpu::point_batch_table::PointBatchSync;
use crate::scene_gpu::primitive_table::PrimitiveLayouts;
use molgfx_core::Scene;
use molgfx_gpu::Device;

pub(super) struct SemanticSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) scene: &'a Scene,
    pub(super) quality: bool,
    pub(super) extent: [u32; 2],
    pub(super) dynamic_sources_changed: bool,
    pub(super) derived_cache: &'a mut crate::DerivedCache,
    pub(super) derived_frame: u64,
}

impl<D: Device> GpuScene<D> {
    pub(super) fn sync_semantic_tables(
        &mut self,
        input: SemanticSync<'_, D>,
    ) -> Result<bool, RenderError> {
        let SemanticSync {
            device,
            queue,
            scene,
            quality,
            extent,
            dynamic_sources_changed,
            derived_cache,
            derived_frame,
        } = input;
        let mut changed = self.visual_fallback.sync(queue);
        let visual_resources = GenericVisualResources {
            programs: &self.visual_programs,
            parameters: &self.visual_parameters,
            properties: &self.visual_properties,
            fallback: self.visual_fallback.entries(),
            parameter_slot_base: self.slots.len(),
            time_seconds: scene.presentation_time_seconds(),
        };
        changed |= self.point_batches.sync(&mut PointBatchSync {
            device,
            queue,
            cull_layout: &self.generic_point_cull_layout,
            render_layout: &self.generic_point_render_layout,
            timeline_layout: &self.instance_timeline_layout,
            scene,
            picking: &self.picking_pages,
            extent,
            visual: visual_resources,
            derived_cache,
            frame: derived_frame,
        })?;
        changed |= self.instance_batches.sync(&mut InstanceBatchSync {
            device,
            queue,
            cull_layout: &self.generic_instance_cull_layout,
            render_layout: &self.generic_instance_render_layout,
            timeline_layout: &self.instance_timeline_layout,
            scene,
            picking: &self.picking_pages,
            visual: visual_resources,
            derived_cache,
            frame: derived_frame,
        })?;
        changed |= self.interactions.sync(&RelationSync {
            device,
            queue,
            render_layout: &self.interaction_layout,
            cull_layout: &self.relation_cull_layout,
            resolve_layout: &self.relation_resolve_layout,
            scene,
            sources: RelationSources {
                structures: &self.structures,
                asset_arena: &self.asset_arena,
                points: &self.point_batches,
                instances: &self.instance_batches,
            },
            picking: &self.picking_pages,
            dynamic_sources_changed,
            visual: visual_resources,
        })?;
        changed |= self.primitive.sync(
            device,
            queue,
            &PrimitiveLayouts {
                table: &self.primitive_layout,
                motion: &self.primitive_motion_layout,
                shadow: &self.primitive_shadow_layout,
            },
            scene,
            &self.structures,
        )?;
        changed |= self.ligand_poses.sync(
            device,
            queue,
            &self.ligand_pose_layout,
            scene,
            &self.structures,
            quality,
        )?;
        changed |= self.labels.sync(
            device,
            queue,
            &self.label_declutter_layout,
            &self.label_render_layout,
            scene,
            &self.structures,
        )?;
        changed |= self
            .overlays
            .sync(device, queue, &self.overlay_layout, scene)?;
        Ok(changed)
    }
}
