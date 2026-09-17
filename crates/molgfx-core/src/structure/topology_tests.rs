use super::*;

fn bond(a: u32, b: u32) -> TopologyBond {
    TopologyBond::new(a, b, false).unwrap_or_else(|error| panic!("{error}"))
}

fn frame(index: u64, time: f32, bonds: Vec<TopologyBond>) -> BondTopologyFrame {
    BondTopologyFrame::new(index, time, 4, bonds.into(), format!("frame-{index}"))
        .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn frames_require_canonical_sorted_unique_connectivity() {
    let duplicate = vec![bond(0, 1), bond(1, 0)];
    assert!(BondTopologyFrame::new(0, 0.0, 4, duplicate.into(), "bad").is_err());
    let reversed = vec![bond(2, 3), bond(0, 1)];
    assert!(BondTopologyFrame::new(0, 0.0, 4, reversed.into(), "bad").is_err());
    assert!(TopologyBond::new(1, 1, false).is_err());
}

#[test]
fn sampling_reuses_one_union_and_scales_birth_and_fracture() {
    let start = frame(0, 0.0, vec![bond(0, 1), bond(1, 2)]);
    let end = frame(1, 1.0, vec![bond(1, 2), bond(2, 3)]);
    let mut segment =
        BondTopologySegment::new(start, end, 0.0).unwrap_or_else(|error| panic!("{error}"));
    let pointer = segment.merged.as_ptr();
    segment
        .set_sample_time(0.25)
        .unwrap_or_else(|error| panic!("{error}"));
    let weights = segment
        .bonds()
        .map(|bond| bond.weight())
        .collect::<Vec<_>>();
    assert_eq!(weights, [0.75, 1.0, 0.25]);
    segment
        .set_sample_time(0.75)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(pointer, segment.merged.as_ptr());
}

#[test]
fn aromatic_state_changes_at_the_midpoint_without_duplicate_geometry() {
    let start = BondTopologyFrame::new(0, 0.0, 2, vec![bond(0, 1)].into(), "start")
        .unwrap_or_else(|error| panic!("{error}"));
    let aromatic = TopologyBond::new(0, 1, true).unwrap_or_else(|error| panic!("{error}"));
    let end = BondTopologyFrame::new(1, 1.0, 2, vec![aromatic].into(), "end")
        .unwrap_or_else(|error| panic!("{error}"));
    let segment =
        BondTopologySegment::new(start, end, 0.5).unwrap_or_else(|error| panic!("{error}"));
    let active = segment.bonds().next().unwrap_or_else(|| panic!("bond"));
    assert!(active.bond().is_aromatic());
    assert_eq!(segment.bonds().len(), 1);
}
