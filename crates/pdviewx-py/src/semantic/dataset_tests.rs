use super::*;

#[test]
fn bond_topology_payload_kind_round_trips_without_an_adapter_default() {
    let python = PyPayloadKind::from(pdviewx::PayloadKind::BondTopology);

    assert_eq!(python, PyPayloadKind::BondTopology);
    assert_eq!(
        pdviewx::PayloadKind::from(python),
        pdviewx::PayloadKind::BondTopology
    );
}
