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

#[test]
fn trajectory_capacity_stays_within_an_eighth_and_under_the_device_limit() {
    use super::trajectory_capacity;
    let limit = 128 << 20;
    let needed = 100_000_000;
    let capacity = trajectory_capacity(needed, limit).expect("fits");
    assert!(capacity >= needed && capacity - needed <= needed / 8);
    let exact = trajectory_capacity(limit - 12, limit).expect("fits");
    assert!(exact <= limit, "never rounds past the limit");
    assert!(trajectory_capacity(limit + 1, limit).is_err());
}

fn scalar_frame(index: u64, time: f64, x: f32) -> molgfx_core::TrajectoryFrame {
    molgfx_core::TrajectoryFrame::new(index, time, std::sync::Arc::from([[x, 0.0, 0.0]]), "frame")
        .expect("frame validates")
}

fn buffer_count(device: &crate::testing::MockDevice) -> usize {
    device.log.buffers.lock().expect("buffers").len()
}

#[test]
fn linear_residency_is_unchanged_and_hermite_adds_only_the_neighbours_it_has() {
    use super::{GpuTrajectory, TrajectoryUniforms};
    use crate::testing::MockDevice;
    use molgfx_core::TrajectorySegment;

    let device = MockDevice::default();
    let queue = device.queue();
    let layout = crate::scene_gpu::layouts::trajectory_layout(&device);
    let mut trajectory = GpuTrajectory::new();
    let linear = TrajectorySegment::new(scalar_frame(1, 1.0, 0.0), scalar_frame(2, 2.0, 2.0), 1.5)
        .expect("segment validates");
    trajectory
        .sync(&device, &queue, &layout, Some(&linear))
        .expect("linear");
    // Two frames, two outputs and the uniforms: the pre-existing five buffers.
    assert_eq!(buffer_count(&device), 5);
    let payloads = device.log.write_payloads.lock().expect("payloads");
    let uniforms =
        bytemuck::pod_read_unaligned::<TrajectoryUniforms>(payloads.last().expect("uniform"));
    assert_eq!(uniforms.mode, 0);
    drop(payloads);

    let hermite = linear
        .clone()
        .with_cubic_hermite(
            Some(scalar_frame(0, 0.0, -1.0)),
            Some(scalar_frame(3, 3.0, 1.0)),
        )
        .expect("neighbours validate");
    trajectory
        .sync(&device, &queue, &layout, Some(&hermite))
        .expect("hermite");
    assert_eq!(buffer_count(&device), 7, "before and after are allocated");
    let payloads = device.log.write_payloads.lock().expect("payloads");
    let uniforms =
        bytemuck::pod_read_unaligned::<TrajectoryUniforms>(payloads.last().expect("uniform"));
    assert_eq!(uniforms.mode, 1);
    let sum: f32 = uniforms.weights.iter().sum();
    assert!((sum - 1.0).abs() < 1.0e-6);
    drop(payloads);

    trajectory
        .sync(&device, &queue, &layout, Some(&linear))
        .expect("back to linear");
    assert!(trajectory.before.is_none() && trajectory.after.is_none());
    assert_eq!(
        buffer_count(&device),
        7,
        "returning to linear allocates nothing"
    );
}

#[test]
fn a_hermite_track_at_the_first_frame_holds_only_the_neighbour_it_has() {
    use super::GpuTrajectory;
    use crate::testing::MockDevice;
    use molgfx_core::TrajectorySegment;

    let device = MockDevice::default();
    let queue = device.queue();
    let layout = crate::scene_gpu::layouts::trajectory_layout(&device);
    let mut trajectory = GpuTrajectory::new();
    let segment = TrajectorySegment::new(scalar_frame(0, 0.0, 0.0), scalar_frame(1, 1.0, 2.0), 0.5)
        .and_then(|segment| segment.with_cubic_hermite(None, Some(scalar_frame(2, 2.0, 1.0))))
        .expect("segment validates");
    trajectory
        .sync(&device, &queue, &layout, Some(&segment))
        .expect("hermite");
    assert!(trajectory.before.is_none() && trajectory.after.is_some());
}
