use super::*;
use crate::{
    ChunkBounds, ChunkDescriptor, ChunkFootprint, ChunkPayload, ChunkSpan, DatasetCatalog,
    LogicalRow, PayloadKind, PropertyChunkPayload, PropertyValues, ProviderDatasetBridge,
    ResidencyClass, ResidencyDetail,
};
use std::sync::Arc;

const HOST_BYTES: u64 = 24;

fn budget() -> ResidencyBudget {
    ResidencyBudget {
        cpu: 256,
        staging: 256,
        gpu_hot: 256,
        gpu_warm: 256,
        in_flight: 256,
    }
}

fn key(dataset: u64, chunk: u64) -> ResidencyKey {
    ResidencyKey {
        dataset: DatasetId::new(dataset),
        chunk: ChunkId::new(chunk),
        detail: ResidencyDetail::Atom,
    }
}

fn request(key: ResidencyKey, priority: i32) -> ResidencyRequest {
    ResidencyRequest {
        key,
        footprint: ChunkFootprint::new(16, HOST_BYTES, 16, 24),
        class: ResidencyClass::Hot,
        priority,
    }
}

fn frame_data(key: ResidencyKey) -> (ChunkData, usize) {
    let source_dataset = match molframe::DatasetDescriptor::regular(
        molframe::DatasetId::new(key.dataset.get()),
        molframe::PayloadKind::Frame,
        1_u64 << 50,
        2,
        molframe::ChunkId::new(key.chunk.get()),
    ) {
        Ok(dataset) => dataset,
        Err(error) => panic!("provider dataset must be valid: {error}"),
    };
    let source_chunk = match source_dataset.regular_chunk(molframe::ChunkId::new(key.chunk.get())) {
        Ok(chunk) => chunk,
        Err(error) => panic!("provider chunk must be valid: {error}"),
    };
    let coordinates: molframe::CoordinateBlock =
        [[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]].into_iter().collect();
    let pointer = coordinates.as_slice().as_ptr() as usize;
    let frame = match molframe::FrameChunk::shared(source_chunk, coordinates, 0..2) {
        Ok(frame) => frame,
        Err(error) => panic!("provider frame must be valid: {error}"),
    };
    let bridge = ProviderDatasetBridge::new(source_dataset);
    let data = match bridge.frame_chunk(frame, request(key, 0).footprint) {
        Ok(data) => data,
        Err(error) => panic!("provider bridge must accept the frame: {error}"),
    };
    (data, pointer)
}

fn property_data(key: ResidencyKey, footprint: ChunkFootprint) -> ChunkData {
    let span = match ChunkSpan::new(LogicalRow::new(u64::from(u32::MAX) + 3), 2) {
        Ok(value) => value,
        Err(error) => panic!("fixture span must be valid: {error}"),
    };
    let bounds = match ChunkBounds::new([0.0; 3], [1.0; 3]) {
        Ok(value) => value,
        Err(error) => panic!("fixture bounds must be valid: {error}"),
    };
    let catalog = match DatasetCatalog::new(
        key.dataset,
        vec![ChunkDescriptor {
            id: key.chunk,
            parent: None,
            level: 0,
            rows: span,
            bounds,
            payload_kind: PayloadKind::ScalarProperty,
            footprint,
        }],
    ) {
        Ok(value) => value,
        Err(error) => panic!("fixture catalog must be valid: {error}"),
    };
    let payload =
        match PropertyChunkPayload::new(PropertyValues::Integer(Arc::from([10, 20])), None) {
            Ok(value) => value,
            Err(error) => panic!("fixture property must be valid: {error}"),
        };
    match ChunkData::new(&catalog, key.chunk, ChunkPayload::Property(payload)) {
        Ok(value) => value,
        Err(error) => panic!("fixture chunk data must be valid: {error}"),
    }
}

fn issue(
    working_set: &mut HostWorkingSet,
    request: ResidencyRequest,
    output: &mut ResidencyOutput,
) -> ResidencyTicket {
    match working_set.request_into(request, output) {
        Ok(ticket) => ticket,
        Err(error) => panic!("request must fit: {error}"),
    }
}

fn retained_pointer(working_set: &HostWorkingSet, key: ResidencyKey) -> usize {
    let Some(data) = working_set.payload(key) else {
        panic!("payload must be retained")
    };
    let ChunkPayload::ProviderFrame(frame) = data.payload() else {
        panic!("provider frame expected")
    };
    frame.positions().as_ptr() as usize
}

#[test]
fn delivery_retains_provider_storage_through_every_resident_phase() {
    let identity = key(u64::from(u32::MAX) + 11, u64::from(u32::MAX) + 19);
    let (payload, pointer) = frame_data(identity);
    let mut working_set = HostWorkingSet::new(budget());
    let mut output = ResidencyOutput::default();
    let ticket = issue(&mut working_set, request(identity, 1), &mut output);

    assert_eq!(output.requests, vec![ticket]);
    assert_eq!(
        working_set.deliver_into(ticket, payload, &mut output),
        Ok(())
    );
    assert_eq!(retained_pointer(&working_set, identity), pointer);
    assert_eq!(
        working_set.residency().snapshot(identity).phase,
        ResidencyPhase::ReadyCpu
    );
    assert_eq!(working_set.begin_upload_into(ticket, &mut output), Ok(()));
    assert_eq!(retained_pointer(&working_set, identity), pointer);
    assert_eq!(
        working_set.complete_upload_into(ticket, &mut output),
        Ok(())
    );
    assert_eq!(retained_pointer(&working_set, identity), pointer);
    assert_eq!(
        working_set.residency().snapshot(identity).phase,
        ResidencyPhase::Resident
    );
}

#[test]
fn stale_delivery_is_typed_and_cancellation_drops_the_current_payload() {
    let identity = key(7, 9);
    let mut working_set = HostWorkingSet::new(budget());
    let mut output = ResidencyOutput::default();
    let first = issue(&mut working_set, request(identity, 1), &mut output);
    let second = issue(&mut working_set, request(identity, 2), &mut output);
    let (stale_payload, _) = frame_data(identity);

    assert!(matches!(
        working_set.deliver_into(first, stale_payload, &mut output),
        Err(HostWorkingSetError::StaleCompletion(stale))
            if stale.ticket == first && stale.current_generation == second.generation()
    ));
    assert_eq!(working_set.retained_payloads(), 0);

    let (current_payload, _) = frame_data(identity);
    assert_eq!(
        working_set.deliver_into(second, current_payload, &mut output),
        Ok(())
    );
    assert_eq!(working_set.cancel_into(second, &mut output), Ok(()));
    assert_eq!(working_set.retained_payloads(), 0);
    assert_eq!(
        working_set.residency().snapshot(identity).phase,
        ResidencyPhase::Absent
    );
}

#[test]
fn identity_mismatches_do_not_advance_or_retain_payloads() {
    let expected = key(21, 31);
    let mut working_set = HostWorkingSet::new(budget());
    let mut output = ResidencyOutput::default();
    let ticket = issue(&mut working_set, request(expected, 1), &mut output);
    let (foreign_dataset, _) = frame_data(key(22, 31));
    assert!(matches!(
        working_set.deliver_into(ticket, foreign_dataset, &mut output),
        Err(HostWorkingSetError::DatasetMismatch { .. })
    ));
    let (foreign_chunk, _) = frame_data(key(21, 32));
    assert!(matches!(
        working_set.deliver_into(ticket, foreign_chunk, &mut output),
        Err(HostWorkingSetError::ChunkMismatch { .. })
    ));
    assert_eq!(working_set.retained_payloads(), 0);
    assert_eq!(
        working_set.residency().snapshot(expected).phase,
        ResidencyPhase::Requested
    );
}

#[test]
fn device_loss_preserves_cpu_storage_and_invalidates_upload_generation() {
    let identity = key(41, 51);
    let (payload, pointer) = frame_data(identity);
    let mut working_set = HostWorkingSet::new(budget());
    let mut output = ResidencyOutput::default();
    let ticket = issue(&mut working_set, request(identity, 1), &mut output);
    assert_eq!(
        working_set.deliver_into(ticket, payload, &mut output),
        Ok(())
    );
    assert_eq!(working_set.begin_upload_into(ticket, &mut output), Ok(()));

    let report = working_set.device_lost_into(&mut output);
    assert_eq!(report.map(|value| value.ready_cpu), Ok(1));
    let Some(retry) = output.ready_uploads.first().copied() else {
        panic!("device loss must emit the replacement upload ticket")
    };
    assert_eq!(working_set.retained_payloads(), 1);
    assert_eq!(retained_pointer(&working_set, identity), pointer);
    assert_eq!(
        working_set.residency().snapshot(identity).phase,
        ResidencyPhase::ReadyCpu
    );
    assert!(matches!(
        working_set.complete_upload_into(ticket, &mut output),
        Err(HostWorkingSetError::StaleCompletion(_))
    ));
    assert_eq!(working_set.begin_upload_into(retry, &mut output), Ok(()));
    assert_eq!(working_set.complete_upload_into(retry, &mut output), Ok(()));
    assert_eq!(retained_pointer(&working_set, identity), pointer);
}

#[test]
fn failure_and_budget_eviction_release_only_the_affected_working_set_entries() {
    let first_key = key(61, u64::from(u32::MAX) + 71);
    let second_key = key(61, u64::from(u32::MAX) + 72);
    let mut working_set = HostWorkingSet::new(budget());
    let mut output = ResidencyOutput::default();
    let first = issue(&mut working_set, request(first_key, 0), &mut output);
    let second = issue(&mut working_set, request(second_key, 1), &mut output);
    let (first_payload, _) = frame_data(first_key);
    let (second_payload, _) = frame_data(second_key);
    assert_eq!(
        working_set.deliver_into(first, first_payload, &mut output),
        Ok(())
    );
    assert_eq!(working_set.begin_upload_into(first, &mut output), Ok(()));
    assert_eq!(working_set.complete_upload_into(first, &mut output), Ok(()));
    assert_eq!(
        working_set.deliver_into(second, second_payload, &mut output),
        Ok(())
    );
    assert_eq!(working_set.begin_upload_into(second, &mut output), Ok(()));
    assert_eq!(
        working_set.complete_upload_into(second, &mut output),
        Ok(())
    );
    assert_eq!(working_set.retained_payloads(), 2);

    let mut constrained = budget();
    constrained.cpu = HOST_BYTES;
    working_set.set_budget_into(constrained, &mut output);
    assert_eq!(working_set.retained_payloads(), 1);
    assert!(working_set.payload(first_key).is_none());
    assert!(working_set.payload(second_key).is_some());
    assert_eq!(
        working_set.fail_into(second, FailureReason::Provider, &mut output),
        Ok(())
    );
    assert_eq!(working_set.retained_payloads(), 0);
}

#[test]
fn cpu_pressure_drops_ready_payloads_without_scanning_logical_chunks() {
    let identity = key(81, 1_u64 << 40);
    let mut working_set = HostWorkingSet::new(budget());
    let mut output = ResidencyOutput::default();
    let ticket = issue(&mut working_set, request(identity, 1), &mut output);
    let (payload, _) = frame_data(identity);
    assert_eq!(
        working_set.deliver_into(ticket, payload, &mut output),
        Ok(())
    );
    assert_eq!(working_set.retained_payloads(), 1);

    let mut constrained = budget();
    constrained.cpu = 0;
    working_set.set_budget_into(constrained, &mut output);

    assert_eq!(working_set.retained_payloads(), 0);
    assert_eq!(working_set.residency().usage().cpu, 0);
    assert_eq!(
        working_set.residency().snapshot(identity).phase,
        ResidencyPhase::Absent
    );
}

#[test]
fn typed_payloads_above_u32_require_catalog_and_ticket_footprints_to_match() {
    let identity = key(u64::from(u32::MAX) + 101, u64::from(u32::MAX) + 103);
    let catalog_footprint = ChunkFootprint::new(4, 16, 8, 16);
    let payload = property_data(identity, catalog_footprint);
    let mut working_set = HostWorkingSet::new(budget());
    let mut output = ResidencyOutput::default();
    let ticket = issue(&mut working_set, request(identity, 1), &mut output);

    assert!(matches!(
        working_set.deliver_into(ticket, payload, &mut output),
        Err(HostWorkingSetError::FootprintMismatch {
            expected: ChunkFootprint {
                host_bytes: HOST_BYTES,
                ..
            },
            actual: ChunkFootprint { host_bytes: 16, .. },
        })
    ));
    assert_eq!(working_set.retained_payloads(), 0);
    assert_eq!(
        working_set.residency().snapshot(identity).phase,
        ResidencyPhase::Requested
    );
}

#[test]
fn cancelled_typed_payload_completion_is_stale_without_retention() {
    let identity = key(91, u64::from(u32::MAX) + 107);
    let footprint = ChunkFootprint::new(4, 16, 8, 16);
    let mut working_set = HostWorkingSet::new(budget());
    let mut output = ResidencyOutput::default();
    let typed_request = ResidencyRequest {
        key: identity,
        footprint,
        class: ResidencyClass::Hot,
        priority: 1,
    };
    let ticket = issue(&mut working_set, typed_request, &mut output);
    assert_eq!(working_set.cancel_into(ticket, &mut output), Ok(()));
    let payload = property_data(identity, footprint);
    assert!(matches!(
        working_set.deliver_into(ticket, payload, &mut output),
        Err(HostWorkingSetError::StaleCompletion(_))
    ));
    assert_eq!(working_set.retained_payloads(), 0);
}
