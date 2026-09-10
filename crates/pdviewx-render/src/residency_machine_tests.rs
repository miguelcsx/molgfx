use super::{ResidencyMachine, ResidencyMachineError, ResidencyState};
use pdviewx_core::{ChunkFootprint, ChunkId};

#[test]
fn lifecycle_reaches_resident_with_real_payload_bytes() {
    let mut machine = ResidencyMachine::new(1);
    let footprint = ChunkFootprint::new(0, 64, 64, 256);
    let Ok(ticket) = machine.request(ChunkId::new(7), footprint) else {
        panic!("machine has one free slot")
    };
    assert!(machine.ready_cpu(ticket).is_ok());
    assert!(machine.uploading(ticket).is_ok());
    assert!(machine.resident(ticket).is_ok());
    assert_eq!(machine.state(ticket), Some(ResidencyState::Resident));
    assert_eq!(machine.metrics().resident_resources, 1);
    assert_eq!(machine.metrics().resident_payload_bytes, 256);
}

#[test]
fn fixed_capacity_reports_backpressure() {
    let mut machine = ResidencyMachine::new(1);
    let footprint = ChunkFootprint::default();
    assert!(machine.request(ChunkId::new(1), footprint).is_ok());
    assert_eq!(
        machine.request(ChunkId::new(2), footprint),
        Err(ResidencyMachineError::Capacity)
    );
    assert_eq!(machine.metrics().capacity_stalls, 1);
}
