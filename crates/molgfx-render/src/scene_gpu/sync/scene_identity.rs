//! Cache reset when one engine begins rendering a distinct scene.

use super::{
    FrameUniforms, FrameUploadCommand, GpuInstanceBatches, GpuInteractions, GpuLabels,
    GpuLigandPoses, GpuOverlays, GpuPointBatches, GpuPrimitives, GpuScene,
};
use crate::{RenderError, ResidencyMetrics, ResidencyState};
use molgfx_gpu::{Device, FenceValue, Queue};

impl<D: Device> GpuScene<D> {
    pub(crate) fn begin_frame(&mut self) {
        self.residency.begin_frame();
        for atlas in &mut self.brick_atlases {
            atlas.begin_frame();
        }
    }

    pub(crate) fn write_frame_uniforms(
        &mut self,
        queue: &D::Queue,
        uniforms: &FrameUniforms,
    ) -> Result<(), RenderError> {
        let bytes = bytemuck::bytes_of(uniforms);
        let reservation = self
            .residency
            .uploads_mut()
            .reserve(bytes.len())
            .map_err(|_| residency_error("frame uniform upload budget exhausted"))?;
        self.residency
            .uploads_mut()
            .bytes_mut(reservation)
            .map_err(|_| residency_error("frame uniform staging range became stale"))?
            .copy_from_slice(bytes);
        self.residency
            .uploads_mut()
            .commit(reservation.ticket())
            .map_err(|_| residency_error("frame uniform staging commit failed"))?;
        let command = FrameUploadCommand {
            ticket: reservation.ticket(),
            offset: reservation.offset(),
            len: reservation.len(),
        };
        if self.residency.commands_mut().push(command).is_err() {
            let _cancelled = self.residency.uploads_mut().cancel(reservation.ticket());
            return Err(residency_error("frame command scratch exhausted"));
        }
        let first_upload = self.residency_machine.state(self.frame_residency_ticket)
            == Some(ResidencyState::ReadyCpu);
        if first_upload {
            self.residency_machine
                .uploading(self.frame_residency_ticket)
                .map_err(|_| residency_error("frame uniform lifecycle rejected upload"))?;
        }
        let start = command.offset;
        let Some(end) = start.checked_add(command.len) else {
            return Err(residency_error("frame uniform staging range overflowed"));
        };
        let Some(staged) = self.residency.uploads().staging_bytes().get(start..end) else {
            return Err(residency_error("frame uniform staging range is invalid"));
        };
        queue.write_buffer(&self.frame_uniforms, 0, staged);
        self.upload_fence = self.upload_fence.wrapping_add(1).max(1);
        let fence = FenceValue(self.upload_fence);
        self.residency
            .uploads_mut()
            .submit(command.ticket, fence)
            .map_err(|_| residency_error("frame uniform in-flight budget exhausted"))?;
        let retired = self.residency.uploads_mut().retire(fence);
        if retired.tickets != 1 {
            return Err(residency_error("frame uniform upload did not retire"));
        }
        if first_upload {
            self.residency_machine
                .resident(self.frame_residency_ticket)
                .map_err(|_| residency_error("frame uniform lifecycle rejected residency"))?;
        }
        Ok(())
    }

    pub(crate) fn residency_metrics(&self) -> ResidencyMetrics {
        let mut metrics = self.residency.metrics();
        metrics.machine = self.residency_machine.metrics();
        metrics.machine.resident_resources = metrics
            .machine
            .resident_resources
            .saturating_add(self.assets.len() as u64);
        metrics.machine.resident_payload_bytes =
            metrics.machine.resident_payload_bytes.saturating_add(
                self.assets
                    .iter()
                    .map(|asset| asset.resident_bytes())
                    .sum::<u64>(),
            );
        metrics.immutable_assets = self.asset_arena.metrics();
        metrics
    }

    pub(super) fn begin_scene(&mut self, identity: u64) -> Result<bool, RenderError> {
        if self.scene_identity == Some(identity) {
            return Ok(false);
        }
        self.scene_identity = Some(identity);
        self.structures.clear();
        self.release_all_assets()?;
        self.slots.clear();
        self.volume_resources.clear();
        self.volume_slots.clear();
        self.mesh_slots.clear();
        self.segmentation_resources.clear();
        self.segmentation_slots.clear();
        self.interactions = GpuInteractions::new();
        self.point_batches = GpuPointBatches::new();
        self.instance_batches = GpuInstanceBatches::new();
        self.primitive = GpuPrimitives::new();
        self.ligand_poses = GpuLigandPoses::new();
        self.labels = GpuLabels::new();
        self.overlays = GpuOverlays::new();
        self.structure_revision = None;
        self.slot_structure_revision = None;
        self.representation_revision = None;
        self.volume_slot_revision = None;
        self.segmentation_slot_revision = None;
        self.mesh_synced = None;
        Ok(true)
    }
}

fn residency_error(reason: &'static str) -> RenderError {
    RenderError::Residency { reason }
}
