use super::*;
use molgfx_core::{ChunkId, ChunkSpan, DatasetId, LogicalRow};
use molgfx_math::{Aabb, Vec3};

fn bound(x: f32) -> Aabb {
    Aabb::new(Vec3::new(x, -0.5, -0.5), Vec3::new(x + 1.0, 0.5, 0.5))
}

fn chunk(id: u64, first: u64, rows: u32) -> SpatialChunk {
    SpatialChunk {
        dataset: DatasetId::new(u64::MAX - 7),
        chunk: ChunkId::new(id),
        rows: ChunkSpan::new(LogicalRow::new(first), rows)
            .unwrap_or_else(|error| panic!("{error}")),
    }
}

#[test]
fn two_level_query_resolves_global_rows_above_u32() {
    let mut index = PagedSpatialIndex::new(2).unwrap_or_else(|error| panic!("{error}"));
    index
        .replace_page(
            0,
            chunk(u64::MAX - 2, u64::from(u32::MAX) + 9, 2),
            &[bound(0.0), bound(2.0)],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    index
        .replace_page(
            1,
            chunk(u64::MAX - 1, u64::from(u32::MAX) + 100, 1),
            &[bound(100.0)],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let maintenance = index.commit().unwrap_or_else(|error| panic!("{error}"));
    let mut candidates = Vec::new();
    index
        .aabb_candidates(bound(1.5), &mut candidates)
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(maintenance.rebuilt);
    assert_eq!(maintenance.resident_bounds_visited, 2);
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.chunk == ChunkId::new(u64::MAX - 2))
    );
    assert!(candidates.iter().any(|candidate| {
        candidate.row.get() == u64::from(u32::MAX) + 10 && candidate.local_row.get() == 1
    }));
}

#[test]
fn refit_preserves_membership_and_moves_candidates() {
    let mut index = PagedSpatialIndex::new(1).unwrap_or_else(|error| panic!("{error}"));
    let token = index
        .replace_page(0, chunk(4, 20, 1), &[bound(0.0)])
        .unwrap_or_else(|error| panic!("{error}"));
    index.commit().unwrap_or_else(|error| panic!("{error}"));
    index
        .refit_page(token, &[bound(20.0)])
        .unwrap_or_else(|error| panic!("{error}"));
    let maintenance = index.commit().unwrap_or_else(|error| panic!("{error}"));
    let mut candidates = Vec::new();
    index
        .sphere_candidates(Vec3::new(20.5, 0.0, 0.0), 1.0, &mut candidates)
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(!maintenance.rebuilt);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].row, LogicalRow::new(20));
}

#[test]
fn queries_reject_uncommitted_changes_and_reused_tokens() {
    let mut index = PagedSpatialIndex::new(1).unwrap_or_else(|error| panic!("{error}"));
    let stale = index
        .replace_page(0, chunk(7, 30, 1), &[bound(0.0)])
        .unwrap_or_else(|error| panic!("{error}"));
    let mut candidates = Vec::new();
    assert!(matches!(
        index.ray_candidates(Vec3::ZERO, Vec3::X, &mut candidates),
        Err(SpatialError::UncommittedChanges)
    ));
    index.commit().unwrap_or_else(|error| panic!("{error}"));
    index.evict(stale).unwrap_or_else(|error| panic!("{error}"));
    index.commit().unwrap_or_else(|error| panic!("{error}"));
    let current = index
        .replace_page(0, chunk(8, 40, 1), &[bound(4.0)])
        .unwrap_or_else(|error| panic!("{error}"));

    assert_ne!(stale.generation(), current.generation());
    assert!(matches!(
        index.refit_page(stale, &[bound(5.0)]),
        Err(SpatialError::StalePage { .. })
    ));
}

#[test]
fn resident_storage_is_fixed_by_capacity_not_logical_identity() {
    let index = PagedSpatialIndex::new(8).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(index.capacity(), 8);
    assert_eq!(index.resident_len(), 0);
}
