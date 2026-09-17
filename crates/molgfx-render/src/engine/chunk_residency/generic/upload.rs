//! Shared bounded upload path for generic provider payloads.

use super::super::payload::{attribute_payload_kind, attribute_target, generic_bytes};
use super::{ChunkGpuResidency, TrackedGenericChunk, ticket_key};
use crate::engine::ChunkResidencyError;
use molgfx_core::{ChunkData, ChunkPayload, ResidencyTicket};
use molgfx_gpu::{Device, Queue as _};

impl<D: Device> ChunkGpuResidency<D> {
    pub(in crate::engine::chunk_residency) fn stage_generic(
        &mut self,
        device: &D,
        queue: &D::Queue,
        ticket: ResidencyTicket,
        data: &ChunkData,
    ) -> Result<(), ChunkResidencyError> {
        if self.tracked.len().saturating_add(self.generic.len()) == self.config.machine_capacity {
            return Err(ChunkResidencyError::TrackingCapacity);
        }
        let (source, stride) = generic_bytes(data.payload())?;
        let target = attribute_target(data.payload());
        let attribute_kind = attribute_payload_kind(data.payload());
        let relation_layout = source.relation_layout();
        let byte_len = source.byte_len()?;
        let allocation = self.arena.allocate(byte_len)?;
        let (cluster_allocation, cluster_count) = match self.point_clusters(data.payload()) {
            Ok(value) => value,
            Err(error) => {
                self.arena.release(allocation)?;
                return Err(error);
            }
        };
        if let Err(error) = self.ensure_source_buffer(device) {
            self.arena.release(allocation)?;
            self.release_generic_cluster(cluster_allocation)?;
            return Err(error);
        }
        if cluster_allocation.is_some()
            && let Err(error) = self
                .ensure_cluster_buffer(device)
                .and_then(|()| self.ensure_display_buffer(device))
        {
            self.arena.release(allocation)?;
            self.release_generic_cluster(cluster_allocation)?;
            return Err(error);
        }
        let reservation = match self
            .uploads
            .ensure()?
            .reserve(usize::try_from(byte_len).map_err(|_| ChunkResidencyError::SizeOverflow)?)
        {
            Ok(value) => value,
            Err(error) => {
                self.arena.release(allocation)?;
                self.release_generic_cluster(cluster_allocation)?;
                return Err(error.into());
            }
        };
        let uploads = self.uploads.ensure()?;
        source.write(uploads.bytes_mut(reservation)?)?;
        uploads.commit(reservation.ticket())?;
        if let Err(error) = uploads.ensure_submittable(reservation.ticket()) {
            uploads.cancel(reservation.ticket())?;
            self.arena.release(allocation)?;
            self.release_generic_cluster(cluster_allocation)?;
            return Err(error.into());
        }
        let start = reservation.offset();
        let end = start.saturating_add(reservation.len());
        queue.write_buffer(
            &self.buffer,
            allocation.byte_offset(),
            &uploads.staging_bytes()[start..end],
        );
        if let Some(cluster_allocation) = cluster_allocation {
            queue.write_buffer(
                &self.display_buffer,
                allocation.byte_offset(),
                &uploads.staging_bytes()[start..end],
            );
            queue.write_buffer(
                &self.cluster_buffer,
                cluster_allocation.byte_offset(),
                bytemuck::cast_slice(self.cluster_scratch.as_slice()),
            );
        }
        let fence = queue.submit_tracked(device.create_command_encoder());
        uploads.submit(reservation.ticket(), fence)?;
        self.insert_generic(&TrackedGenericChunk::Uploading {
            ticket,
            allocation,
            cluster_allocation,
            cluster_count,
            fence,
            kind: data.payload().kind(),
            target,
            span: data.span(),
            byte_len,
            stride,
            attribute_kind,
            relation_layout,
            cancelled: false,
        });
        if let ChunkPayload::RelationBatch(payload) = data.payload() {
            self.relation_sources.insert(
                ticket_key(ticket),
                std::sync::Arc::clone(payload.relations()),
            );
        }
        Ok(())
    }
}
