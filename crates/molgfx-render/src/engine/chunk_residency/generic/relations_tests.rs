use super::*;
use molgfx_core::{
    ChunkEntityRef, ChunkId, ChunkOccurrenceId, ChunkSpatialKind, DatasetId, LogicalRow,
    PagedRelation, PagedSpatialAnchor,
};

#[test]
fn occurrence_identity_is_packed_without_changing_homogeneous_row_order() {
    let start = entity(17, 41);
    let end = entity(29, 43);
    let rows = [PagedRelation {
        start: PagedSpatialAnchor::Entity(start),
        end: PagedSpatialAnchor::Entity(end),
    }];
    let upload = RelationUpload::new(&rows)
        .unwrap_or_else(|error| panic!("homogeneous relation must lower: {error}"));
    assert_eq!(upload.layout().uniform_stride(), 72);
    let byte_len = usize::try_from(upload.byte_len())
        .unwrap_or_else(|error| panic!("test upload must fit host addressing: {error}"));
    let mut bytes = vec![0_u8; byte_len];
    upload
        .write(&mut bytes)
        .unwrap_or_else(|error| panic!("relation bytes must fit: {error}"));
    assert_eq!(read_u64(&bytes, 16), 17);
    assert_eq!(read_u64(&bytes, 52), 29);
}

fn entity(occurrence: u64, row: u64) -> ChunkEntityRef {
    ChunkEntityRef::new(
        DatasetId::new(7),
        ChunkId::new(11),
        ChunkOccurrenceId::new(occurrence),
        LogicalRow::new(row),
        ChunkSpatialKind::Point,
    )
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let Some(value) = bytes.get(offset..offset + 8) else {
        panic!("test offset must be in bounds")
    };
    let mut packed = [0_u8; 8];
    packed.copy_from_slice(value);
    u64::from_le_bytes(packed)
}
