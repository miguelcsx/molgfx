use super::{LazyUploadRing, ResidencyConfig, ResidencyWorkspace};
use pdviewx_gpu::{FenceValue, UploadRingConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DrawCommand(u32);

#[test]
fn lazy_upload_ring_retains_no_staging_bytes_before_first_use() {
    let config = ResidencyConfig::default().uploads;
    let mut ring = match LazyUploadRing::new(config) {
        Ok(value) => value,
        Err(error) => panic!("valid lazy ring must initialize: {error}"),
    };
    assert_eq!(ring.metrics().host_allocation_events, 0);
    let initialized = match ring.ensure() {
        Ok(value) => value,
        Err(error) => panic!("first use must allocate staging: {error}"),
    };
    assert_eq!(initialized.metrics().host_allocation_events, 2);
    assert_eq!(ring.metrics().host_allocation_events, 2);
}

fn workspace() -> ResidencyWorkspace<DrawCommand> {
    let result = ResidencyWorkspace::new(ResidencyConfig {
        page_size: 256,
        page_count: 16,
        uploads: UploadRingConfig {
            capacity_bytes: 256,
            ticket_capacity: 8,
            epoch_budget_bytes: 128,
            in_flight_budget_bytes: 256,
            alignment: 16,
        },
        command_capacity: 8,
        machine_capacity: 8,
    });
    let Ok(workspace) = result else {
        panic!("workspace configuration is valid")
    };
    workspace
}

#[test]
fn frame_reset_retains_every_backing_allocation() {
    let mut workspace = workspace();
    let initial = workspace.metrics();
    for frame in 1..65 {
        workspace.begin_frame();
        assert!(workspace.commands_mut().push(DrawCommand(frame)).is_ok());
        let Ok(reservation) = workspace.uploads_mut().reserve(64) else {
            panic!("frame upload fits")
        };
        assert!(workspace.uploads_mut().commit(reservation.ticket()).is_ok());
        assert!(
            workspace
                .uploads_mut()
                .submit(reservation.ticket(), FenceValue(u64::from(frame)))
                .is_ok()
        );
        assert_eq!(
            workspace
                .uploads_mut()
                .retire(FenceValue(u64::from(frame)))
                .tickets,
            1
        );
    }
    let final_metrics = workspace.metrics();
    assert_eq!(
        final_metrics.arena.host_allocation_events,
        initial.arena.host_allocation_events
    );
    assert_eq!(
        final_metrics.uploads.host_allocation_events,
        initial.uploads.host_allocation_events
    );
    assert_eq!(
        final_metrics.commands.host_allocation_events,
        initial.commands.host_allocation_events
    );
}

#[test]
fn workspace_reports_bytes_stalls_and_command_pressure() {
    let mut workspace = workspace();
    assert!(workspace.arena_mut().allocate(4_097).is_err());
    assert!(workspace.uploads_mut().reserve(129).is_err());
    for command in 0..8 {
        assert!(workspace.commands_mut().push(DrawCommand(command)).is_ok());
    }
    assert!(workspace.commands_mut().push(DrawCommand(9)).is_err());
    let metrics = workspace.metrics();
    assert_eq!(metrics.arena.stalled_bytes, 4_097);
    assert_eq!(metrics.uploads.stalled_bytes, 129);
    assert_eq!(metrics.commands.capacity_stalls, 1);
}
