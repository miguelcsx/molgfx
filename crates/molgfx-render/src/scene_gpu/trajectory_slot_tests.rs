use super::trajectory_dispatch_groups;

#[test]
fn trajectory_dispatch_covers_all_three_coordinate_lanes_per_atom() {
    assert_eq!(trajectory_dispatch_groups(1), [1, 1]);
    assert_eq!(trajectory_dispatch_groups(21), [1, 1]);
    assert_eq!(trajectory_dispatch_groups(22), [2, 1]);
    assert_eq!(trajectory_dispatch_groups(64), [3, 1]);
    assert_eq!(trajectory_dispatch_groups(1_398_080), [65_535, 1]);
    assert_eq!(trajectory_dispatch_groups(1_398_081), [32_768, 2]);
    assert_eq!(trajectory_dispatch_groups(2_440_800), [57_207, 2]);
}

#[test]
fn reactivating_a_resident_pair_starts_without_motion_from_the_old_activation() {
    use super::{GpuTrajectory, TrajectoryUniforms};
    use crate::testing::MockDevice;
    use molgfx_core::{TrajectoryFrame, TrajectorySegment};
    use std::sync::Arc;

    let device = MockDevice::default();
    let queue = device.queue();
    let layout = crate::scene_gpu::layouts::trajectory_layout(&device);
    let mut trajectory = GpuTrajectory::new();
    let start = TrajectoryFrame::new(0, 0.0, Arc::from([[0.0, 0.0, 0.0]]), "start")
        .expect("start frame validates");
    let end = TrajectoryFrame::new(1, 1.0, Arc::from([[4.0, 0.0, 0.0]]), "end")
        .expect("end frame validates");
    let mut segment = TrajectorySegment::new(start, end, 0.25).expect("segment validates");
    trajectory
        .sync(&device, &queue, &layout, Some(&segment))
        .expect("first activation");
    trajectory
        .sync(&device, &queue, &layout, None)
        .expect("deactivation");
    let writes_before = device.log.writes.lock().expect("writes").len();
    let buffers_before = device.log.buffers.lock().expect("buffers").len();
    segment.set_sample_time(0.75).expect("new sample validates");
    trajectory
        .sync(&device, &queue, &layout, Some(&segment))
        .expect("reactivation");
    assert_eq!(
        device.log.buffers.lock().expect("buffers").len(),
        buffers_before
    );
    assert_eq!(
        device.log.writes.lock().expect("writes").len(),
        writes_before + 1
    );
    let payloads = device.log.write_payloads.lock().expect("payloads");
    let uniforms =
        bytemuck::pod_read_unaligned::<TrajectoryUniforms>(payloads.last().expect("uniform write"));
    assert_eq!(uniforms.alpha.to_bits(), 0.75_f32.to_bits());
    assert_eq!(
        uniforms.previous_alpha.to_bits(),
        0.75_f32.to_bits(),
        "the old activation is not a previous frame"
    );
    drop(payloads);
    let writes_before = device.log.writes.lock().expect("writes").len();
    trajectory
        .sync(&device, &queue, &layout, Some(&segment))
        .expect("unchanged sample");
    assert_eq!(
        device.log.writes.lock().expect("writes").len(),
        writes_before,
        "reactivation requires no settling upload"
    );

    segment.set_sample_time(1.0).expect("next sample validates");
    trajectory
        .sync(&device, &queue, &layout, Some(&segment))
        .expect("continuous playback");
    let payloads = device.log.write_payloads.lock().expect("payloads");
    let uniforms =
        bytemuck::pod_read_unaligned::<TrajectoryUniforms>(payloads.last().expect("uniform write"));
    assert_eq!(uniforms.alpha.to_bits(), 1.0_f32.to_bits());
    assert_eq!(
        uniforms.previous_alpha.to_bits(),
        0.75_f32.to_bits(),
        "continuous playback retains real motion"
    );
    drop(payloads);
    trajectory
        .sync(&device, &queue, &layout, None)
        .expect("moving trajectory detaches before settling");
    trajectory
        .sync(&device, &queue, &layout, Some(&segment))
        .expect("same sample reactivates");
    let payloads = device.log.write_payloads.lock().expect("payloads");
    let uniforms =
        bytemuck::pod_read_unaligned::<TrajectoryUniforms>(payloads.last().expect("uniform write"));
    assert_eq!(uniforms.previous_alpha.to_bits(), 1.0_f32.to_bits());
}
