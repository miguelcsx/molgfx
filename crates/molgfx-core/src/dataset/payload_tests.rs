use super::*;
use crate::{
    BrickAddress, BrickId, BrickMetadata, BrickShape, BrickValueRange, ChunkBounds,
    ChunkDescriptor, ChunkFootprint, ChunkSpan, DatasetCatalog, DirtyGeneration, LogicalRow,
    PropertyValues,
};
use std::sync::Arc;

fn descriptor(kind: PayloadKind, rows: u32) -> ChunkDescriptor {
    ChunkDescriptor {
        id: ChunkId::new(4),
        parent: None,
        level: 0,
        rows: match ChunkSpan::new(LogicalRow::new(0), rows) {
            Ok(span) => span,
            Err(error) => panic!("fixture span rejected: {error}"),
        },
        bounds: match ChunkBounds::new([0.0; 3], [1.0; 3]) {
            Ok(bounds) => bounds,
            Err(error) => panic!("fixture bounds rejected: {error}"),
        },
        payload_kind: kind,
        footprint: ChunkFootprint::new(0, 4096, 0, 0),
    }
}

#[test]
fn structure_payload_retains_shared_columns() {
    let positions: Arc<[[f32; 3]]> = Arc::from([[1.0, 2.0, 3.0]]);
    let pointer = positions.as_ptr();
    let payload = match StructureChunkPayload::new(
        Arc::clone(&positions),
        Arc::from([6]),
        Arc::from([9]),
        Arc::from([1.7]),
    ) {
        Ok(payload) => payload,
        Err(error) => panic!("valid payload rejected: {error}"),
    };
    assert_eq!(payload.positions().as_ptr(), pointer);
}

#[test]
fn chunk_data_checks_kind_and_row_count() {
    let scalar = match ScalarChunkPayload::new(Arc::from([1.0, 2.0])) {
        Ok(payload) => ChunkPayload::ScalarProperty(payload),
        Err(error) => panic!("valid payload rejected: {error}"),
    };
    let proxy_catalog =
        match DatasetCatalog::new(DatasetId::new(1), vec![descriptor(PayloadKind::Proxy, 2)]) {
            Ok(catalog) => catalog,
            Err(error) => panic!("fixture catalog rejected: {error}"),
        };
    assert!(matches!(
        ChunkData::new(&proxy_catalog, ChunkId::new(4), scalar.clone()),
        Err(DatasetError::PayloadKindMismatch { .. })
    ));
    let scalar_catalog = match DatasetCatalog::new(
        DatasetId::new(1),
        vec![descriptor(PayloadKind::ScalarProperty, 1)],
    ) {
        Ok(catalog) => catalog,
        Err(error) => panic!("fixture catalog rejected: {error}"),
    };
    assert!(matches!(
        ChunkData::new(&scalar_catalog, ChunkId::new(4), scalar.clone()),
        Err(DatasetError::PayloadRowCountMismatch { .. })
    ));
    let matching_catalog = match DatasetCatalog::new(
        DatasetId::new(8),
        vec![descriptor(PayloadKind::ScalarProperty, 2)],
    ) {
        Ok(catalog) => catalog,
        Err(error) => panic!("fixture catalog rejected: {error}"),
    };
    let data = match ChunkData::new(&matching_catalog, ChunkId::new(4), scalar.clone()) {
        Ok(data) => data,
        Err(error) => panic!("matching payload rejected: {error}"),
    };
    assert_eq!(data.dataset_id(), DatasetId::new(8));
    assert_eq!(data.chunk_id(), ChunkId::new(4));
    assert!(matches!(
        ChunkData::new(&matching_catalog, ChunkId::new(99), scalar),
        Err(DatasetError::MissingChunk { .. })
    ));
}

#[test]
fn mesh_indices_must_be_local_to_the_chunk() {
    let result = MeshChunkPayload::new(
        Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
        Arc::from([[0.0, 0.0, 1.0]; 3]),
        Arc::from([[255, 255, 255, 255]; 3]),
        Arc::from([0, 1, 3]),
    );
    assert!(matches!(result, Err(DatasetError::InvalidPayload { .. })));
}

#[test]
fn bricks_validate_halo_and_exact_storage() {
    let shape = match BrickShape::new([4; 3], 1) {
        Ok(value) => value,
        Err(error) => panic!("fixture shape rejected: {error}"),
    };
    let scalar = match BrickMetadata::new(
        BrickId::new(7),
        BrickAddress {
            origin: [0; 3],
            mip: 0,
        },
        shape,
        BrickValueRange::Scalar { min: 0.0, max: 0.0 },
        DirtyGeneration::new(1),
    ) {
        Ok(value) => value,
        Err(error) => panic!("fixture metadata rejected: {error}"),
    };
    let result = VolumeBrickPayload::new(scalar, Arc::from([0.0; 64]));
    assert!(result.is_ok());
    let invalid = LabelBrickPayload::new(scalar, Arc::from([0; 64]));
    assert!(matches!(invalid, Err(DatasetError::InvalidPayload { .. })));
}

#[test]
fn occupancy_bricks_are_bit_packed_without_expansion() {
    let shape = match BrickShape::new([4; 3], 1) {
        Ok(value) => value,
        Err(error) => panic!("fixture shape rejected: {error}"),
    };
    let metadata = match BrickMetadata::new(
        BrickId::new(8),
        BrickAddress {
            origin: [0; 3],
            mip: 0,
        },
        shape,
        BrickValueRange::Occupancy {
            has_empty: false,
            has_occupied: true,
        },
        DirtyGeneration::new(2),
    ) {
        Ok(value) => value,
        Err(error) => panic!("fixture metadata rejected: {error}"),
    };
    let payload = match OccupancyBrickPayload::new(metadata, Arc::from([u64::MAX])) {
        Ok(value) => value,
        Err(error) => panic!("valid occupancy rejected: {error}"),
    };
    assert_eq!(payload.voxel_count(), 64);
    assert_eq!(payload.words().len(), 1);
}

#[test]
fn typed_properties_preserve_physical_storage_and_missing_values() {
    let values: Arc<[f64]> = Arc::from([1.5, f64::NAN, 3.5]);
    let pointer = values.as_ptr();
    let payload =
        match PropertyChunkPayload::new(PropertyValues::Real(values), Some(Arc::from([0b101]))) {
            Ok(value) => value,
            Err(error) => panic!("valid typed property rejected: {error}"),
        };
    let PropertyValues::Real(retained) = payload.values() else {
        panic!("real storage expected")
    };
    assert_eq!(retained.as_ptr(), pointer);
    assert_eq!(payload.row_count(), 3);
    assert!(matches!(
        PropertyChunkPayload::new(
            PropertyValues::Boolean {
                rows: 65,
                words: Arc::from([0, 2]),
            },
            None,
        ),
        Err(DatasetError::InvalidPayload { .. })
    ));
}

#[test]
fn trajectory_batches_keep_only_frame_major_working_set_coordinates() {
    let positions: Arc<[[f32; 3]]> = Arc::from([
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ]);
    let pointer = positions.as_ptr();
    let payload = match TrajectoryFramesPayload::new(
        u64::from(u32::MAX) + 5,
        Arc::from([0.0, 0.1]),
        2,
        positions,
    ) {
        Ok(value) => value,
        Err(error) => panic!("valid frame batch rejected: {error}"),
    };
    assert_eq!(payload.positions().as_ptr(), pointer);
    assert_eq!(payload.rows_per_frame(), 2);
    assert_eq!(payload.first_frame(), u64::from(u32::MAX) + 5);
}

#[test]
fn catalog_rejects_payloads_whose_retained_bytes_are_underdeclared() {
    let mut underdeclared = descriptor(PayloadKind::Mesh, 3);
    underdeclared.footprint.host_bytes = 1;
    let catalog = match DatasetCatalog::new(DatasetId::new(3), vec![underdeclared]) {
        Ok(value) => value,
        Err(error) => panic!("fixture catalog rejected: {error}"),
    };
    let mesh = match MeshChunkPayload::new(
        Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
        Arc::from([[0.0, 0.0, 1.0]; 3]),
        Arc::from([[255, 255, 255, 255]; 3]),
        Arc::from([0, 1, 2]),
    ) {
        Ok(value) => value,
        Err(error) => panic!("fixture mesh rejected: {error}"),
    };
    assert!(matches!(
        ChunkData::new(&catalog, ChunkId::new(4), ChunkPayload::Mesh(mesh)),
        Err(DatasetError::PayloadFootprintTooSmall { .. })
    ));
}
