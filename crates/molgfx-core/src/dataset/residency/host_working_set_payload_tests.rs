use super::*;
use crate::{
    BrickAddress, BrickId, BrickMetadata, BrickShape, BrickValueRange, ChunkBounds, ChunkData,
    ChunkDescriptor, ChunkFootprint, ChunkPayload, ChunkSpan, DatasetCatalog, DatasetId,
    DirtyGeneration, LogicalRow, MeshChunkPayload, OccupancyBrickPayload, PayloadKind,
    PropertyChunkPayload, PropertyValues, ProxyChunkPayload, ResidencyClass, ResidencyDetail,
    TrajectoryFramesPayload, VolumeBrickPayload,
};
use std::sync::Arc;

#[test]
fn every_typed_payload_family_is_bounded_and_cancellable_above_u32() {
    let payloads = typed_payload_fixtures();
    let mut working_set = HostWorkingSet::new(ResidencyBudget {
        cpu: 256,
        staging: 256,
        gpu_hot: 256,
        gpu_warm: 256,
        in_flight: 256,
    });
    let mut output = ResidencyOutput::default();
    for (offset, (kind, payload, rows, host_bytes)) in payloads.into_iter().enumerate() {
        let key = ResidencyKey {
            dataset: DatasetId::new(u64::from(u32::MAX) + 211),
            chunk: crate::ChunkId::new(u64::from(u32::MAX) + 223 + offset as u64),
            detail: ResidencyDetail::Atom,
        };
        let footprint = ChunkFootprint::new(0, host_bytes, 0, 0);
        let data = catalog_data(key, kind, rows, footprint, payload);
        let ticket = match working_set.request_into(
            ResidencyRequest {
                key,
                footprint,
                class: ResidencyClass::Hot,
                priority: 1,
            },
            &mut output,
        ) {
            Ok(value) => value,
            Err(error) => panic!("fixture request rejected: {error}"),
        };
        assert_eq!(working_set.deliver_into(ticket, data, &mut output), Ok(()));
        assert_eq!(working_set.retained_payloads(), 1);
        assert_eq!(working_set.cancel_into(ticket, &mut output), Ok(()));
        assert_eq!(working_set.retained_payloads(), 0);
    }
}

fn catalog_data(
    identity: ResidencyKey,
    kind: PayloadKind,
    rows: u32,
    footprint: ChunkFootprint,
    payload: ChunkPayload,
) -> ChunkData {
    let span = match ChunkSpan::new(LogicalRow::new(u64::from(u32::MAX) + 17), rows) {
        Ok(value) => value,
        Err(error) => panic!("fixture span rejected: {error}"),
    };
    let bounds = match ChunkBounds::new([0.0; 3], [1.0; 3]) {
        Ok(value) => value,
        Err(error) => panic!("fixture bounds rejected: {error}"),
    };
    let catalog = match DatasetCatalog::new(
        identity.dataset,
        vec![ChunkDescriptor {
            id: identity.chunk,
            parent: None,
            level: 0,
            rows: span,
            bounds,
            payload_kind: kind,
            footprint,
        }],
    ) {
        Ok(value) => value,
        Err(error) => panic!("fixture catalog rejected: {error}"),
    };
    match ChunkData::new(&catalog, identity.chunk, payload) {
        Ok(value) => value,
        Err(error) => panic!("fixture payload rejected: {error}"),
    }
}

fn typed_payload_fixtures() -> Vec<(PayloadKind, ChunkPayload, u32, u64)> {
    vec![
        property_fixture(),
        trajectory_fixture(),
        volume_fixture(),
        occupancy_fixture(),
        mesh_fixture(),
        proxy_fixture(),
    ]
}

fn property_fixture() -> (PayloadKind, ChunkPayload, u32, u64) {
    let value = match PropertyChunkPayload::new(
        PropertyValues::Symbol(Arc::from([7, 9])),
        Some(Arc::from([0b11])),
    ) {
        Ok(value) => value,
        Err(error) => panic!("property fixture rejected: {error}"),
    };
    (
        PayloadKind::ScalarProperty,
        ChunkPayload::Property(value),
        2,
        16,
    )
}

fn trajectory_fixture() -> (PayloadKind, ChunkPayload, u32, u64) {
    let value = match TrajectoryFramesPayload::new(
        u64::from(u32::MAX) + 1,
        Arc::from([0.0, 1.0]),
        2,
        Arc::from([[0.0; 3]; 4]),
    ) {
        Ok(value) => value,
        Err(error) => panic!("trajectory fixture rejected: {error}"),
    };
    (
        PayloadKind::Trajectory,
        ChunkPayload::TrajectoryFrames(value),
        2,
        64,
    )
}

fn volume_fixture() -> (PayloadKind, ChunkPayload, u32, u64) {
    let metadata = scalar_brick_metadata(301, [2, 1, 1]);
    let value = match VolumeBrickPayload::new(metadata, Arc::from([0.0, 1.0])) {
        Ok(value) => value,
        Err(error) => panic!("volume fixture rejected: {error}"),
    };
    (
        PayloadKind::VolumeBrick,
        ChunkPayload::VolumeBrick(value),
        2,
        8,
    )
}

fn occupancy_fixture() -> (PayloadKind, ChunkPayload, u32, u64) {
    let shape = match BrickShape::new([65, 1, 1], 0) {
        Ok(value) => value,
        Err(error) => panic!("occupancy shape rejected: {error}"),
    };
    let metadata = match BrickMetadata::new(
        BrickId::new(303),
        BrickAddress {
            origin: [0; 3],
            mip: 0,
        },
        shape,
        BrickValueRange::Occupancy {
            has_empty: true,
            has_occupied: true,
        },
        DirtyGeneration::new(1),
    ) {
        Ok(value) => value,
        Err(error) => panic!("occupancy metadata rejected: {error}"),
    };
    let value = match OccupancyBrickPayload::new(metadata, Arc::from([1, 0])) {
        Ok(value) => value,
        Err(error) => panic!("occupancy fixture rejected: {error}"),
    };
    (
        PayloadKind::LabelBrick,
        ChunkPayload::OccupancyBrick(value),
        65,
        16,
    )
}

fn mesh_fixture() -> (PayloadKind, ChunkPayload, u32, u64) {
    let value = match MeshChunkPayload::new(
        Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
        Arc::from([[0.0, 0.0, 1.0]; 3]),
        Arc::from([[255; 4]; 3]),
        Arc::from([0, 1, 2]),
    ) {
        Ok(value) => value,
        Err(error) => panic!("mesh fixture rejected: {error}"),
    };
    (PayloadKind::Mesh, ChunkPayload::Mesh(value), 3, 96)
}

fn proxy_fixture() -> (PayloadKind, ChunkPayload, u32, u64) {
    let value = match ProxyChunkPayload::new(
        Arc::from([[0.0; 3], [1.0; 3]]),
        Arc::from([1.0, 2.0]),
        Arc::from([[255; 4]; 2]),
    ) {
        Ok(value) => value,
        Err(error) => panic!("proxy fixture rejected: {error}"),
    };
    (PayloadKind::Proxy, ChunkPayload::Proxy(value), 2, 40)
}

fn scalar_brick_metadata(id: u64, stored: [u16; 3]) -> BrickMetadata {
    let shape = match BrickShape::new(stored, 0) {
        Ok(value) => value,
        Err(error) => panic!("volume shape rejected: {error}"),
    };
    match BrickMetadata::new(
        BrickId::new(id),
        BrickAddress {
            origin: [0; 3],
            mip: 0,
        },
        shape,
        BrickValueRange::Scalar { min: 0.0, max: 1.0 },
        DirtyGeneration::new(1),
    ) {
        Ok(value) => value,
        Err(error) => panic!("volume metadata rejected: {error}"),
    }
}
