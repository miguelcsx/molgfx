use super::*;
use crate::LodLevel;
use crate::streaming::test_support::handle;

fn plan<'a>(
    planner: &mut StreamPlanner,
    requests: &[ChunkRequest],
    output: &'a mut StreamPlan,
) -> &'a StreamPlan {
    planner.plan_into(requests, output);
    output
}

#[test]
fn stream_planner_retains_high_priority_chunks_under_a_byte_cap() {
    let mut planner = StreamPlanner::new(StreamingBudget {
        max_resident_bytes: 10,
        max_requests_per_frame: 4,
    });
    let requests = [
        ChunkRequest {
            key: ChunkKey {
                structure: handle(),
                level: LodLevel::Residue,
                index: 2,
            },
            priority: 1.0,
            bytes: 6,
        },
        ChunkRequest {
            key: ChunkKey {
                structure: handle(),
                level: LodLevel::Residue,
                index: 1,
            },
            priority: 2.0,
            bytes: 4,
        },
    ];
    let mut output = StreamPlan::default();
    let plan = plan(&mut planner, &requests, &mut output);
    assert_eq!(plan.retain.len(), 2);
    assert_eq!(plan.retain[0].key.index, 1);
    assert_eq!(plan.resident_bytes, 10);
}

#[test]
fn planner_reports_evictions_when_a_visible_set_changes() {
    let structure = handle();
    let mut planner = StreamPlanner::new(StreamingBudget {
        max_resident_bytes: 32,
        max_requests_per_frame: 4,
    });
    let first = ChunkRequest {
        key: ChunkKey {
            structure,
            level: LodLevel::Domain,
            index: 0,
        },
        priority: 1.0,
        bytes: 8,
    };
    let mut output = StreamPlan::default();
    assert!(plan(&mut planner, &[first], &mut output).evict.is_empty());
    let second = ChunkRequest {
        key: ChunkKey {
            index: 1,
            ..first.key
        },
        ..first
    };
    let plan = plan(&mut planner, &[second], &mut output);
    assert_eq!(plan.evict, vec![first.key]);
}

#[test]
fn planner_caps_new_requests_but_keeps_resident_visible_chunks() {
    let structure = handle();
    let mut planner = StreamPlanner::new(StreamingBudget {
        max_resident_bytes: 32,
        max_requests_per_frame: 1,
    });
    let first = ChunkRequest {
        key: ChunkKey {
            structure,
            level: LodLevel::Residue,
            index: 0,
        },
        priority: 1.0,
        bytes: 8,
    };
    let second = ChunkRequest {
        key: ChunkKey {
            index: 1,
            ..first.key
        },
        priority: 2.0,
        bytes: 8,
    };
    let mut output = StreamPlan::default();
    assert_eq!(
        plan(&mut planner, &[first, second], &mut output).retain,
        vec![second]
    );
    let plan = plan(&mut planner, &[first, second], &mut output);
    assert_eq!(plan.retain, vec![second, first]);
}

#[test]
fn planner_reuses_intermediate_and_output_capacity_after_warmup() {
    let structure = handle();
    let requests = (0u16..64)
        .map(|index| ChunkRequest {
            key: ChunkKey {
                structure,
                level: LodLevel::Residue,
                index: u32::from(index),
            },
            priority: f32::from(index),
            bytes: 8,
        })
        .collect::<Vec<_>>();
    let mut planner = StreamPlanner::new(StreamingBudget {
        max_resident_bytes: 512,
        max_requests_per_frame: 64,
    });
    let mut output = StreamPlan::default();
    planner.plan_into(&requests, &mut output);
    let scratch = planner.scratch_capacities();
    let capacities = (output.retain.capacity(), output.evict.capacity());
    planner.plan_into(&requests, &mut output);
    assert_eq!(scratch, planner.scratch_capacities());
    assert_eq!(
        capacities,
        (output.retain.capacity(), output.evict.capacity())
    );
    assert_eq!(output.retain.len(), 64);
}

#[test]
fn cloned_planner_preserves_residency_without_copying_scratch() {
    let structure = handle();
    let first = ChunkRequest {
        key: ChunkKey {
            structure,
            level: LodLevel::Residue,
            index: 0,
        },
        priority: 1.0,
        bytes: 8,
    };
    let second = ChunkRequest {
        key: ChunkKey {
            index: 1,
            ..first.key
        },
        ..first
    };
    let mut planner = StreamPlanner::new(StreamingBudget {
        max_resident_bytes: 16,
        max_requests_per_frame: 1,
    });
    let mut output = StreamPlan::default();
    assert_eq!(
        plan(&mut planner, &[first], &mut output).retain,
        vec![first]
    );
    let mut cloned = planner.clone();
    let (ordered, resident, selected) = cloned.scratch_capacities();
    assert_eq!((ordered, selected), (0, 0));
    assert!(resident > 0);
    let mut cloned_output = StreamPlan::default();
    assert_eq!(
        plan(&mut cloned, &[first, second], &mut cloned_output).retain,
        vec![first, second]
    );
}
