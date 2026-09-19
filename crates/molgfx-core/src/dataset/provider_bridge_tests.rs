use super::*;
use std::sync::Arc;

fn footprint() -> ChunkFootprint {
    ChunkFootprint::new(0, 128, 0, 64)
}

fn annotated_structure() -> molframe::Structure {
    let structure = crate::fixture::structure();
    let mut editor = structure.edit();
    let values = (0..structure.atom_count()).map(f64::from).collect();
    let Ok(column) = molframe::AnnotationColumn::from_values(values) else {
        return structure;
    };
    if editor
        .set_annotation("score", molframe::AtomAnnotation::Real(column))
        .is_err()
    {
        return structure;
    }
    match editor.commit() {
        Ok(edited) => edited,
        Err(_) => structure,
    }
}

#[test]
fn structure_chunks_preserve_pointer_identity_and_lifetime_above_u32() {
    let dataset = molframe::DatasetId::new(u64::from(u32::MAX) + 17);
    let first_chunk = molframe::ChunkId::new(u64::from(u32::MAX) + 31);
    let structure = crate::fixture::structure();
    let pointer = structure.positions().as_ptr();
    let provider =
        match molframe::StructureChunkProvider::new(dataset, first_chunk, structure.clone()) {
            Ok(provider) => provider,
            Err(error) => panic!("provider must be valid: {error}"),
        };
    let source = match provider.chunk(first_chunk) {
        Ok(chunk) => chunk,
        Err(error) => panic!("chunk must exist: {error}"),
    };
    let bridge = ProviderDatasetBridge::new(provider.dataset());
    let data = match bridge.structure_chunk(source, footprint()) {
        Ok(data) => data,
        Err(error) => panic!("bridge must preserve the native chunk: {error}"),
    };
    drop(provider);
    drop(structure);

    assert_eq!(data.dataset_id().get(), dataset.get());
    assert_eq!(data.chunk_id().get(), first_chunk.get());
    let ChunkPayload::ProviderStructure(shared) = data.payload() else {
        panic!("native structure payload expected");
    };
    assert_eq!(shared.positions().as_ptr(), pointer);
    assert!(!shared.positions().is_empty());
}

#[test]
fn property_and_frame_chunks_keep_native_backing_storage() {
    let structure = annotated_structure();
    let property_pointer = match structure.annotations().get("score") {
        Some(molframe::AtomAnnotation::Real(column)) => column.values().as_ptr(),
        _ => panic!("real property expected"),
    };
    let coordinate_pointer = structure.positions().as_ptr();
    let property_provider = match molframe::PropertyChunkProvider::new(
        molframe::DatasetId::new(41),
        molframe::ChunkId::new(51),
        structure.clone(),
        Arc::<str>::from("score"),
    ) {
        Ok(provider) => provider,
        Err(error) => panic!("property provider must be valid: {error}"),
    };
    let frame_provider = match molframe::FrameChunkProvider::new(
        molframe::DatasetId::new(42),
        molframe::ChunkId::new(61),
        structure,
        molframe::ModelIndex::new(0),
    ) {
        Ok(provider) => provider,
        Err(error) => panic!("frame provider must be valid: {error}"),
    };
    let property_bridge = ProviderDatasetBridge::new(property_provider.dataset());
    let frame_bridge = ProviderDatasetBridge::new(frame_provider.dataset());
    let property = match property_provider.chunk(molframe::ChunkId::new(51)) {
        Ok(chunk) => chunk,
        Err(error) => panic!("property chunk must exist: {error}"),
    };
    let frame = match frame_provider.chunk(molframe::ChunkId::new(61)) {
        Ok(chunk) => chunk,
        Err(error) => panic!("frame chunk must exist: {error}"),
    };
    let property = match property_bridge.property_chunk(property, footprint()) {
        Ok(data) => data,
        Err(error) => panic!("property bridge must succeed: {error}"),
    };
    let frame = match frame_bridge.frame_chunk(frame, footprint()) {
        Ok(data) => data,
        Err(error) => panic!("frame bridge must succeed: {error}"),
    };
    drop(property_provider);
    drop(frame_provider);

    let ChunkPayload::ProviderProperty(shared_property) = property.payload() else {
        panic!("native property payload expected");
    };
    let Some(values) = shared_property.reals() else {
        panic!("real property storage expected");
    };
    let ChunkPayload::ProviderFrame(shared_frame) = frame.payload() else {
        panic!("native frame payload expected");
    };
    assert_eq!(values.as_ptr(), property_pointer);
    assert_eq!(shared_frame.positions().as_ptr(), coordinate_pointer);
}

#[test]
fn bond_chunks_preserve_global_endpoints_and_validate_host_footprint() {
    let structure = crate::fixture::structure();
    let coordinates = structure.positions().as_ptr();
    let atom_dataset = molframe::DatasetId::new(u64::from(u32::MAX) + 300);
    let atom_start = molframe::LogicalRow::new(u64::from(u32::MAX) + 700);
    let provider = match molframe::BondChunkProvider::with_rows_per_chunk(
        molframe::DatasetId::new(43),
        molframe::ChunkId::new(u64::from(u32::MAX) + 60),
        atom_dataset,
        atom_start,
        structure,
        1,
    ) {
        Ok(provider) => provider,
        Err(error) => panic!("bond provider must be valid: {error}"),
    };
    let source = match provider.chunk(provider.dataset().first_chunk()) {
        Ok(chunk) => chunk,
        Err(error) => panic!("bond chunk must exist: {error}"),
    };
    let bounds = match ChunkBounds::new([-1.0; 3], [2.0; 3]) {
        Ok(bounds) => bounds,
        Err(error) => panic!("bounds must be valid: {error}"),
    };
    let bridge = ProviderDatasetBridge::new(provider.dataset());
    assert!(matches!(
        bridge.bond_chunk(source.clone(), bounds, ChunkFootprint::new(0, 0, 0, 0)),
        Err(ProviderBridgeError::Dataset(
            DatasetError::PayloadFootprintTooSmall { .. }
        ))
    ));

    let data = match bridge.bond_chunk(source, bounds, footprint()) {
        Ok(data) => data,
        Err(error) => panic!("bond bridge must preserve native storage: {error}"),
    };
    let ChunkPayload::ProviderBond(shared) = data.payload() else {
        panic!("native bond payload expected");
    };
    let record = match shared.record(molframe::LocalRow::new(0)) {
        Ok(record) => record,
        Err(error) => panic!("global bond record must resolve: {error}"),
    };

    assert_eq!(data.payload().kind(), PayloadKind::BondTopology);
    assert_eq!(record.atom_a.dataset(), atom_dataset);
    assert!(record.atom_a.row().get() >= atom_start.get());
    assert_eq!(shared.structure().positions().as_ptr(), coordinates);
}

#[test]
fn compact_catalog_does_not_enumerate_more_than_u32_chunks() {
    let descriptor = match molframe::DatasetDescriptor::regular(
        molframe::DatasetId::new(71),
        molframe::PayloadKind::Frame,
        1_u64 << 40,
        1,
        molframe::ChunkId::new(1_u64 << 48),
    ) {
        Ok(descriptor) => descriptor,
        Err(error) => panic!("large descriptor must be valid: {error}"),
    };
    let bridge = ProviderDatasetBridge::new(descriptor);

    assert_eq!(bridge.catalog().len(), 0);
    assert!(bridge.catalog().logical_chunk_count() > u64::from(u32::MAX));
    assert!(matches!(
        bridge.catalog().total_footprint(),
        Err(DatasetError::ProviderFootprintUnavailable)
    ));
}

#[test]
fn kind_mismatches_are_typed() {
    let structure = crate::fixture::structure();
    let structure_descriptor = match molframe::DatasetDescriptor::source_defined(
        molframe::DatasetId::new(81),
        molframe::PayloadKind::Structure,
        u64::from(structure.atom_count()),
        1,
        molframe::ChunkId::new(91),
        structure.atom_count(),
    ) {
        Ok(descriptor) => descriptor,
        Err(error) => panic!("descriptor must be valid: {error}"),
    };
    let frame_provider = match molframe::FrameChunkProvider::new(
        molframe::DatasetId::new(81),
        molframe::ChunkId::new(91),
        structure,
        molframe::ModelIndex::new(0),
    ) {
        Ok(provider) => provider,
        Err(error) => panic!("frame provider must be valid: {error}"),
    };
    let frame = match frame_provider.chunk(molframe::ChunkId::new(91)) {
        Ok(chunk) => chunk,
        Err(error) => panic!("frame chunk must exist: {error}"),
    };

    assert!(matches!(
        ProviderDatasetBridge::new(structure_descriptor).frame_chunk(frame, footprint()),
        Err(ProviderBridgeError::PayloadKindMismatch {
            expected: molframe::PayloadKind::Structure,
            actual: molframe::PayloadKind::Frame
        })
    ));
}

#[test]
fn row_range_mismatches_are_typed() {
    let coordinates: molframe::CoordinateBlock =
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]].into_iter().collect();
    let source = match molframe::ChunkDescriptor::new(
        molframe::DatasetId::new(101),
        molframe::ChunkId::new(111),
        molframe::LogicalRow::new(0),
        2,
    ) {
        Ok(descriptor) => descriptor,
        Err(error) => panic!("source descriptor must be valid: {error}"),
    };
    let frame = match molframe::FrameChunk::shared(source, coordinates, 0..2) {
        Ok(frame) => frame,
        Err(error) => panic!("frame must be valid: {error}"),
    };
    let dataset = match molframe::DatasetDescriptor::source_defined(
        molframe::DatasetId::new(101),
        molframe::PayloadKind::Frame,
        1,
        1,
        molframe::ChunkId::new(111),
        2,
    ) {
        Ok(descriptor) => descriptor,
        Err(error) => panic!("dataset descriptor must be valid: {error}"),
    };

    assert!(matches!(
        ProviderDatasetBridge::new(dataset).frame_chunk(frame, footprint()),
        Err(ProviderBridgeError::RowRangeMismatch { .. })
    ));
}
