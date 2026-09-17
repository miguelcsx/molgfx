use super::*;

#[test]
fn bond_topology_payload_kind_round_trips_without_an_adapter_default() {
    let python = PyPayloadKind::from(molgfx::PayloadKind::BondTopology);

    assert_eq!(python, PyPayloadKind::BondTopology);
    assert_eq!(
        molgfx::PayloadKind::from(python),
        molgfx::PayloadKind::BondTopology
    );
}
