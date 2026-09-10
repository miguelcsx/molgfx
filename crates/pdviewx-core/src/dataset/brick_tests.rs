use super::*;

fn metadata(id: u64, origin: [u64; 3], mip: u16) -> BrickMetadata {
    let shape = match BrickShape::new([10, 10, 10], 1) {
        Ok(value) => value,
        Err(error) => panic!("fixture shape rejected: {error}"),
    };
    match BrickMetadata::new(
        BrickId::new(id),
        BrickAddress { origin, mip },
        shape,
        BrickValueRange::Scalar {
            min: -1.0,
            max: 3.0,
        },
        DirtyGeneration::new(4),
    ) {
        Ok(value) => value,
        Err(error) => panic!("fixture metadata rejected: {error}"),
    }
}

#[test]
fn halo_shape_reports_stored_and_interior_voxels() {
    let shape = match BrickShape::new([10, 12, 14], 2) {
        Ok(value) => value,
        Err(error) => panic!("valid halo rejected: {error}"),
    };
    assert_eq!(shape.stored(), [10, 12, 14]);
    assert_eq!(shape.interior(), [6, 8, 10]);
    assert_eq!(shape.halo(), 2);
    assert_eq!(shape.voxel_count(), 1_680);
    assert!(matches!(
        BrickShape::new([4, 8, 8], 2),
        Err(DatasetError::InvalidBrickShape)
    ));
}

#[test]
fn terabyte_logical_brick_catalog_retains_only_sparse_metadata() {
    let descriptor = BrickDescriptor {
        chunk: ChunkId::new(u64::from(u32::MAX) + 9),
        metadata: metadata(u64::from(u32::MAX) + 17, [0; 3], 0),
    };
    let catalog = match BrickCatalog::new(
        DatasetId::new(88),
        [65_536, 65_536, 64],
        4,
        vec![descriptor],
    ) {
        Ok(value) => value,
        Err(error) => panic!("logical catalog rejected: {error}"),
    };
    assert_eq!(catalog.logical_bytes(), Ok(1 << 40));
    assert_eq!(catalog.descriptors().len(), 1);
    assert!(catalog.descriptors()[0].metadata.id.get() > u64::from(u32::MAX));
    assert!(catalog.descriptors()[0].chunk.get() > u64::from(u32::MAX));
}

#[test]
fn dirty_generation_overflow_is_typed() {
    assert_eq!(
        DirtyGeneration::new(u64::MAX).next(),
        Err(DatasetError::GenerationExhausted)
    );
}
