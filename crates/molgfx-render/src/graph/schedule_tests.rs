use super::*;
use crate::graph::node::{PassKind, ResourceId};
use crate::testing::MockDevice;

fn noop(_: &mut crate::graph::PassContext<'_, MockDevice>) {}

fn node(name: &'static str, reads: &[u32], writes: &[u32]) -> PassNode<MockDevice> {
    PassNode {
        name,
        reads: reads.iter().map(|&r| ResourceId(r)).collect(),
        writes: writes.iter().map(|&w| ResourceId(w)).collect(),
        kind: PassKind::Graphics,
        record: noop,
    }
}

#[test]
fn writers_run_before_their_readers() {
    let passes = vec![node("consumer", &[0], &[1]), node("producer", &[], &[0])];
    let order = match schedule(&passes) {
        Ok(order) => order,
        Err(e) => panic!("schedules: {e}"),
    };
    assert_eq!(order, vec![1, 0]);
}

#[test]
fn independent_passes_keep_declaration_order() {
    let passes = vec![
        node("a", &[], &[0]),
        node("b", &[], &[1]),
        node("c", &[], &[2]),
    ];
    let order = match schedule(&passes) {
        Ok(order) => order,
        Err(e) => panic!("schedules: {e}"),
    };
    assert_eq!(order, vec![0, 1, 2], "the tie-break is declaration order");
}

#[test]
fn writers_of_one_resource_keep_declaration_order() {
    let passes = vec![
        node("first_writer", &[], &[0]),
        node("second_writer", &[], &[0]),
        node("reader", &[0], &[]),
    ];
    let order = match schedule(&passes) {
        Ok(order) => order,
        Err(e) => panic!("schedules: {e}"),
    };
    assert_eq!(order, vec![0, 1, 2]);
}

#[test]
fn a_dependency_cycle_is_a_typed_error_not_a_hang() {
    let passes = vec![
        node("ouroboros_head", &[1], &[0]),
        node("ouroboros_tail", &[0], &[1]),
    ];
    let Err(e) = schedule(&passes) else {
        panic!("a cycle must not schedule")
    };
    assert_eq!(e.code(), "MOLGFX-E0070");
}

#[test]
fn a_diamond_schedules_deterministically() {
    // top → left, right → bottom; left and right tie-break by declaration.
    let passes = vec![
        node("top", &[], &[0]),
        node("left", &[0], &[1]),
        node("right", &[0], &[2]),
        node("bottom", &[1, 2], &[3]),
    ];
    let order = match schedule(&passes) {
        Ok(order) => order,
        Err(e) => panic!("schedules: {e}"),
    };
    assert_eq!(order, vec![0, 1, 2, 3]);
}
