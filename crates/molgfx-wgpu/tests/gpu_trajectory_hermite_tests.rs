//! The cubic Hermite trajectory pass must match the CPU blend weights on a real device.
#![cfg(not(target_arch = "wasm32"))]

use molgfx_core::{TrajectoryFrame, TrajectorySegment};
use molgfx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, BufferDesc, BufferUsage,
    CommandEncoder as _, ComputePassDesc, ComputePipelineDesc, Device, DeviceDesc, Opened,
    Queue as _, ShaderModuleDesc, ShaderStages,
};
use molgfx_render::GpuTrajectory;
use molgfx_wgpu::WgpuDevice;
use std::sync::Arc;

/// The trajectory pass bindings: two frames, two outputs, uniforms, two neighbours.
fn trajectory_layout(device: &WgpuDevice) -> <WgpuDevice as Device>::BindGroupLayout {
    let entry = |binding, ty| BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty,
    };
    let read = BindingType::Storage { read_only: true };
    let write = BindingType::Storage { read_only: false };
    device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "trajectory interpolation",
        entries: &[
            entry(0, read),
            entry(1, read),
            entry(2, write),
            entry(3, write),
            entry(4, BindingType::Uniform),
            entry(5, read),
            entry(6, read),
        ],
    })
}

/// Four frames at non-uniform times following one smooth curve per atom.
fn curve_frame(index: u64, time: f64) -> TrajectoryFrame {
    let positions: Vec<[f32; 3]> = (0..5_u32)
        .map(|atom| {
            let phase = f64::from(atom) * 0.7;
            [
                (time * 0.6 + phase).sin() * 2.0,
                (time * time * 0.15 - phase).cos(),
                time * 0.25 + f64::from(atom),
            ]
            .map(|value| molgfx_core::narrow_to_f32(value).expect("coordinate is finite"))
        })
        .collect();
    TrajectoryFrame::new(index, time, Arc::from(positions), "curve").expect("frame validates")
}

/// Dispatches the trajectory pass on a real device and reads both outputs back.
fn dispatch_and_read(
    gpu: &Opened<WgpuDevice>,
    trajectory: &mut GpuTrajectory<WgpuDevice>,
    layout: &<WgpuDevice as Device>::BindGroupLayout,
    pipeline: &<WgpuDevice as Device>::Pipeline,
    segment: &TrajectorySegment,
) -> (Vec<f32>, Vec<f32>) {
    trajectory
        .sync(&gpu.device, &gpu.queue, layout, Some(segment))
        .expect("trajectory syncs");
    let bytes = (segment.atom_count() * 12) as u64;
    let mut encoder = gpu.device.create_command_encoder();
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "trajectory check",
            timestamps: None,
        });
        trajectory.record(&mut pass, pipeline);
    }
    let mut read = Vec::new();
    for source in [trajectory.output(), trajectory.previous_output()] {
        let source = source.expect("active trajectory has outputs");
        let staging = gpu
            .device
            .create_buffer(&BufferDesc {
                label: "trajectory check staging",
                size: bytes,
                usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
            })
            .expect("staging allocates");
        encoder.copy_buffer_to_buffer(source, 0, &staging, 0, bytes);
        read.push(staging);
    }
    gpu.queue.submit(encoder);
    let mut floats = read.iter().map(|staging| {
        let raw = gpu
            .queue
            .read_buffer_blocking(&gpu.device, staging, 0, bytes)
            .expect("outputs read back");
        raw.as_chunks::<4>()
            .0
            .iter()
            .map(|word| f32::from_le_bytes(*word))
            .collect()
    });
    let current = floats.next().expect("current output");
    let previous = floats.next().expect("previous output");
    (current, previous)
}

fn weighted(segment: &TrajectorySegment, weights: [f64; 4]) -> Vec<f64> {
    let frames = [
        segment.before().unwrap_or(segment.start()),
        segment.start(),
        segment.end(),
        segment.after().unwrap_or(segment.end()),
    ];
    (0..segment.atom_count() * 3)
        .map(|index| {
            frames
                .iter()
                .zip(weights)
                .map(|(frame, weight)| weight * f64::from(frame.positions()[index / 3][index % 3]))
                .sum()
        })
        .collect()
}

fn worst_error(gpu: &[f32], cpu: &[f64]) -> f64 {
    assert_eq!(gpu.len(), cpu.len());
    gpu.iter()
        .zip(cpu)
        .map(|(gpu, cpu)| (f64::from(*gpu) - cpu).abs())
        .fold(0.0, f64::max)
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn cubic_hermite_gpu_matches_cpu_weights_on_device() {
    let gpu = WgpuDevice::open_blocking(&DeviceDesc::default(), None).expect("device opens");
    eprintln!("adapters: {:?}", molgfx_wgpu::system_info().adapters);
    let layout = trajectory_layout(&gpu.device);
    let shader = gpu
        .device
        .create_shader_module(&ShaderModuleDesc {
            label: "trajectory check",
            wgsl: molgfx_shaders::TRAJECTORY,
        })
        .expect("trajectory shader compiles");
    let pipeline = gpu
        .device
        .create_compute_pipeline(&ComputePipelineDesc {
            label: "trajectory check",
            layouts: &[Some(&layout)],
            shader: &shader,
            entry: "interpolate_trajectory",
        })
        .expect("trajectory pipeline builds");
    let mut trajectory = GpuTrajectory::new();

    // Times 0, 1, 3, 4: the segment is [1, 3], so the gaps are 1, 2 and 1.
    let mut segment = TrajectorySegment::new(curve_frame(1, 1.0), curve_frame(2, 3.0), 2.0)
        .and_then(|segment| {
            segment.with_cubic_hermite(Some(curve_frame(0, 0.0)), Some(curve_frame(3, 4.0)))
        })
        .expect("segment validates");
    dispatch_and_read(&gpu, &mut trajectory, &layout, &pipeline, &segment);
    segment.set_sample_time(2.5).expect("sample validates");
    let (current, previous) =
        dispatch_and_read(&gpu, &mut trajectory, &layout, &pipeline, &segment);
    assert!((segment.interpolation() - 0.75).abs() < 1.0e-12);
    let now = weighted(&segment, segment.blend_weights(0.75));
    let before = weighted(&segment, segment.blend_weights(0.5));
    let linear = weighted(&segment, [0.0, 0.25, 0.75, 0.0]);
    let (error, previous_error) = (worst_error(&current, &now), worst_error(&previous, &before));
    let separation = worst_error(&current, &linear);
    eprintln!("hermite max error current={error:e} previous={previous_error:e}");
    assert!(error < 1.0e-5 && previous_error < 1.0e-5);
    assert!(
        separation > 1.0e-3,
        "the spline must differ from linear here"
    );

    // The same device and slot, back to a linear segment of the same frames.
    let mut linear_segment = TrajectorySegment::new(curve_frame(1, 1.0), curve_frame(2, 3.0), 2.0)
        .expect("segment validates");
    linear_segment
        .set_sample_time(2.5)
        .expect("sample validates");
    let (current, previous) =
        dispatch_and_read(&gpu, &mut trajectory, &layout, &pipeline, &linear_segment);
    let now = weighted(&linear_segment, [0.0, 0.25, 0.75, 0.0]);
    let (error, previous_error) = (worst_error(&current, &now), worst_error(&previous, &now));
    eprintln!("linear max error current={error:e} previous={previous_error:e}");
    assert!(error < 1.0e-5 && previous_error < 1.0e-5);
}
