use super::*;
use crate::{ChunkBounds, ChunkSpan, LogicalRow, PayloadKind};

fn descriptor(id: u64, parent: Option<u64>, level: u16) -> ChunkDescriptor {
    ChunkDescriptor {
        id: ChunkId::new(id),
        parent: parent.map(ChunkId::new),
        level,
        rows: match ChunkSpan::new(LogicalRow::new(id * 100), 10) {
            Ok(rows) => rows,
            Err(error) => panic!("fixture span rejected: {error}"),
        },
        bounds: match ChunkBounds::new([0.0; 3], [1.0; 3]) {
            Ok(bounds) => bounds,
            Err(error) => panic!("fixture bounds rejected: {error}"),
        },
        payload_kind: PayloadKind::Proxy,
        footprint: ChunkFootprint::new(1, 2, 3, 4),
    }
}

#[test]
fn catalog_sorts_lookup_and_children_deterministically() {
    let catalog = match DatasetCatalog::new(
        DatasetId::new(7),
        vec![
            descriptor(3, Some(1), 1),
            descriptor(1, None, 0),
            descriptor(2, Some(1), 1),
        ],
    ) {
        Ok(catalog) => catalog,
        Err(error) => panic!("valid catalog rejected: {error}"),
    };
    assert_eq!(catalog.dataset_id(), DatasetId::new(7));
    assert_eq!(
        catalog
            .roots()
            .map(|entry| entry.id.get())
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(
        catalog
            .children(ChunkId::new(1))
            .map(|entry| entry.id.get())
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(
        catalog.get(ChunkId::new(2)).map(|entry| entry.level),
        Some(1)
    );
    assert_eq!(
        catalog.total_footprint(),
        Ok(ChunkFootprint::new(3, 6, 9, 12))
    );
}

#[test]
fn catalog_rejects_missing_parents_and_bad_levels() {
    assert!(matches!(
        DatasetCatalog::new(DatasetId::new(1), vec![descriptor(2, Some(9), 1)]),
        Err(DatasetError::MissingParent { .. })
    ));
    assert!(matches!(
        DatasetCatalog::new(
            DatasetId::new(1),
            vec![descriptor(1, None, 0), descriptor(2, Some(1), 2)]
        ),
        Err(DatasetError::InvalidHierarchyLevel { .. })
    ));
}

#[test]
fn terabyte_logical_catalog_stores_metadata_without_payload_materialization() {
    const TIB: u64 = 1 << 40;
    let mut root = descriptor(1, None, 0);
    root.footprint = ChunkFootprint::new(TIB, 64 << 20, 32 << 20, 16 << 20);
    let catalog = match DatasetCatalog::new(DatasetId::new(91), vec![root]) {
        Ok(catalog) => catalog,
        Err(error) => panic!("terabyte metadata catalog rejected: {error}"),
    };

    assert_eq!(catalog.len(), 1);
    assert_eq!(
        catalog.total_footprint().map(|value| value.source_bytes),
        Ok(TIB)
    );
    assert_eq!(catalog.roots().len(), 1);
}
