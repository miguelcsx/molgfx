use super::*;
use molgfx_gpu::{
    BufferDesc, BufferUsage, CommandEncoder as _, Device as _, DeviceDesc, LoadOp, Queue as _,
    TextureDesc, TextureDimension, TextureFormat, TextureUsage, TextureViewDesc,
};

#[test]
#[ignore = "requires a native GPU adapter with timestamp queries"]
fn completed_profiles_resolve_the_last_render_pass_after_sampling_finishes() {
    use molgfx_render::{AdaptiveQualityConfig, Engine, EngineConfig, ImageConfig};
    let config = EngineConfig {
        width: 128,
        height: 72,
        adaptive: AdaptiveQualityConfig::highest_fixed(120),
        ..Default::default()
    };
    let mut engine = Engine::<WgpuDevice>::new(&config, None).unwrap();
    let scene = molgfx_core::Scene::new();
    let camera = molgfx_math::Camera::framing(
        &molgfx_math::BoundingSphere {
            center: molgfx_math::Vec3::ZERO,
            radius: 10.0,
        },
        128.0 / 72.0,
    );
    for output in 0..3 {
        let config = ImageConfig {
            width: 128 + output * 8,
            height: 72,
        };
        let converged = molgfx_render::MeasuredOutput::Converged;
        let timing = if output == 1 {
            pollster::block_on(engine.profile_frame_async(&scene, &camera, config, converged))
                .unwrap()
        } else {
            engine
                .profile_frame(&scene, &camera, config, converged)
                .unwrap()
        };
        assert!(timing.quality.complete());
        assert_eq!(timing.quality.samples_completed, Some(64));
        let last = engine
            .last_pass_timings()
            .last()
            .expect("final render pass captured");
        assert_eq!(last.label, "HDR tonemap");
        assert_eq!(last.sample, Some(63));
        let [start, end] = last.timestamp_ticks.expect("final raw timestamps");
        assert!(start > 0 && end >= start);
        assert!(last.gpu_timing.nanoseconds().is_some());
    }
}
#[test]
#[ignore = "requires a native GPU adapter"]
fn inline_attachment_descriptors_preserve_every_color_target() {
    let opened = WgpuDevice::open_blocking(&DeviceDesc::default(), None).expect("device opens");
    let device = opened.device;
    let queue = opened.queue;
    let readback = device
        .create_buffer(&BufferDesc {
            label: "attachment readback",
            size: 256,
            usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
        })
        .expect("readback allocates");
    for count in [1, 5, 8] {
        let textures: Vec<_> = (0..count)
            .map(|_| {
                device
                    .create_texture(&TextureDesc {
                        label: "inline color target",
                        width: 1,
                        height: 1,
                        depth: 1,
                        dimension: TextureDimension::D2,
                        // Eight R32Uint targets fit the portable 32-byte sample budget.
                        format: TextureFormat::R32Uint,
                        usage: TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::COPY_SRC),
                    })
                    .expect("color target allocates")
            })
            .collect();
        let views: Vec<_> = textures
            .iter()
            .map(|texture| device.create_texture_view(texture, &TextureViewDesc::default()))
            .collect();
        let colors: Vec<_> = views
            .iter()
            .map(|view| ColorAttachment {
                view,
                load: LoadOp::Clear([1.0, 0.0, 0.0, 1.0]),
            })
            .collect();
        let mut encoder = device.create_command_encoder();
        drop(encoder.begin_render_pass(&RenderPassDesc {
            label: "inline attachment conversion",
            colors: &colors,
            depth: None,
            timestamps: None,
        }));
        queue.submit(encoder);
        for texture in &textures {
            let mut copy = device.create_command_encoder();
            copy.copy_texture_to_buffer(texture, (0, 0), (1, 1), 256, 0, &readback);
            queue.submit(copy);
            let bytes = queue
                .read_buffer_blocking(&device, &readback, 0, 4)
                .expect("color target reads back");
            assert_eq!(bytes, [1, 0, 0, 0]);
        }
    }
}

#[test]
#[ignore = "requires a native GPU adapter with timestamp queries"]
fn actual_pass_capture_resolves_unique_pairs_without_changing_rendered_output() {
    use molgfx_gpu::{
        PassTimestampAbsence, PassTimestampCapture, TimestampCaptureIncomplete, TimestampWrites,
    };
    let opened = WgpuDevice::open_blocking(&DeviceDesc::default(), None).expect("device opens");
    let device = opened.device;
    let queue = opened.queue;
    let capture = PassTimestampCapture::new(&device, 5).expect("timestamp capture supported");
    let explicit = device
        .create_timestamp_query_set(1)
        .expect("explicit query allocates");
    let (resolve, readback, texture) = timestamp_targets(&device);
    let view = device.create_texture_view(&texture, &TextureViewDesc::default());
    let (work, group, pipeline) = timestamp_compute_workload(&device);
    let mut encoder = device.create_command_encoder();
    encoder.set_timestamp_capture(Some(capture));
    record_repeated_timestamp_work(&mut encoder, &view, &pipeline, &group);
    record_timestamp_work(
        &mut encoder,
        &ComputePassDesc {
            label: "descriptor timestamps",
            timestamps: Some(TimestampWrites {
                queries: &explicit,
                beginning: None,
                end: Some(0),
            }),
        },
        &pipeline,
        &group,
    );
    drop(encoder.begin_compute_pass(&ComputePassDesc {
        label: "overflow",
        timestamps: None,
    }));
    let capture = encoder.set_timestamp_capture(None).expect("capture drains");
    assert_eq!(capture.query_range(), 0..8);
    assert_eq!(
        capture.records()[4].queries,
        Err(PassTimestampAbsence::DescriptorTimestampWrites)
    );
    assert_eq!(
        capture.incomplete(),
        Some(TimestampCaptureIncomplete::CapacityExceeded {
            capacity: 5,
            omitted_passes: 1
        })
    );
    encoder.resolve_query_set(
        capture.queries().expect("query capture"),
        capture.query_range(),
        &resolve,
        0,
    );
    encoder.resolve_query_set(&explicit, 0..1, &resolve, 256);
    encoder.copy_buffer_to_buffer(&resolve, 0, &readback, 0, 512);
    encoder.copy_texture_to_buffer(&texture, (0, 0), (1, 1), 256, 512, &readback);
    encoder.copy_buffer_to_buffer(&work, 4095 * 4, &readback, 516, 4);
    queue.submit(encoder);
    let bytes = queue
        .read_buffer_blocking(&device, &readback, 0, 520)
        .expect("queries and target read back");
    let ticks: Vec<_> = bytes[..64]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| u64::from_le_bytes(*b))
        .collect();
    assert_eq!(&bytes[512..516], &2_u32.to_le_bytes());
    assert_eq!(&bytes[516..520], &12286_u32.to_le_bytes());
    device
        .check_errors()
        .expect("render and dispatched compute validate");
    eprintln!("nonempty actual passes: timestamps={ticks:?}; render=2; compute=12286");
    assert_actual_timestamp_records(&capture, &ticks);
    let explicit_tick = u64::from_le_bytes(bytes[256..264].try_into().expect("explicit tick"));
    assert!(
        explicit_tick > 0,
        "caller-authored end-only query was preserved"
    );
    assert_eq!(&bytes[512..516], &2_u32.to_le_bytes());
    device.check_errors().expect("no backend validation errors");
    eprintln!("actual pass timestamps: {ticks:?}; explicit={explicit_tick}; output=2");
    verify_timestamp_reuse(
        &device,
        &queue,
        capture,
        (&resolve, &readback),
        (&pipeline, &group),
    );
}

fn record_repeated_timestamp_work(
    encoder: &mut WgpuCommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &WgpuPipeline,
    group: &wgpu::BindGroup,
) {
    for sample in 0..2 {
        encoder.set_timestamp_sample(Some(sample));
        drop(encoder.begin_render_pass(&RenderPassDesc {
            label: "repeated render",
            colors: &[ColorAttachment {
                view,
                load: LoadOp::Clear([f64::from(sample + 1), 0.0, 0.0, 0.0]),
            }],
            depth: None,
            timestamps: None,
        }));
        record_timestamp_work(
            encoder,
            &ComputePassDesc {
                label: "repeated compute",
                timestamps: None,
            },
            pipeline,
            group,
        );
    }
}

fn verify_timestamp_reuse(
    device: &WgpuDevice,
    queue: &super::super::queue::WgpuQueue,
    mut capture: PassTimestampCapture<wgpu::QuerySet>,
    buffers: (&WgpuBuffer, &WgpuBuffer),
    workload: (&WgpuPipeline, &wgpu::BindGroup),
) {
    capture.reset();
    let mut next = device.create_command_encoder();
    next.set_timestamp_capture(Some(capture));
    record_timestamp_work(
        &mut next,
        &ComputePassDesc {
            label: "reuse",
            timestamps: None,
        },
        workload.0,
        workload.1,
    );
    let capture = next
        .set_timestamp_capture(None)
        .expect("reused capture drains");
    assert_eq!(capture.query_range(), 0..2);
    assert_eq!(capture.records()[0].sample, None);
    next.resolve_query_set(
        capture.queries().expect("query capture"),
        capture.query_range(),
        buffers.0,
        0,
    );
    next.copy_buffer_to_buffer(buffers.0, 0, buffers.1, 0, 16);
    queue.submit(next);
    let bytes = queue
        .read_buffer_blocking(device, buffers.1, 0, 16)
        .expect("reused queries read back");
    assert!(u64::from_le_bytes(bytes[..8].try_into().expect("reused beginning")) > 0);
    device.check_errors().expect("reuse validates");
}

fn timestamp_targets(device: &WgpuDevice) -> (WgpuBuffer, WgpuBuffer, WgpuTexture) {
    let resolve = device
        .create_buffer(&BufferDesc {
            label: "pass capture resolve",
            size: 512,
            usage: BufferUsage::QUERY_RESOLVE.union(BufferUsage::COPY_SRC),
        })
        .expect("resolve allocates");
    let readback = device
        .create_buffer(&BufferDesc {
            label: "pass capture readback",
            size: 768,
            usage: BufferUsage::MAP_READ.union(BufferUsage::COPY_DST),
        })
        .expect("readback allocates");
    let texture = device
        .create_texture(&TextureDesc {
            label: "capture output",
            width: 1,
            height: 1,
            depth: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Uint,
            usage: TextureUsage::RENDER_ATTACHMENT.union(TextureUsage::COPY_SRC),
        })
        .expect("target allocates");
    (resolve, readback, texture)
}

fn assert_actual_timestamp_records(capture: &PassTimestampCapture<wgpu::QuerySet>, ticks: &[u64]) {
    for (index, record) in (0_u32..).zip(&capture.records()[..4]) {
        let pair = record.queries.expect("pass captured");
        assert_eq!(pair.beginning, index * 2);
        assert_eq!(pair.end, index * 2 + 1);
        assert_eq!(record.sample, Some(index / 2));
        assert_eq!(
            record.kind,
            if index % 2 == 0 {
                TimestampPassKind::Render
            } else {
                TimestampPassKind::Compute
            }
        );
        assert!(ticks[pair.beginning as usize] > 0);
        assert!(ticks[pair.end as usize] >= ticks[pair.beginning as usize]);
    }
}

fn timestamp_compute_workload(device: &WgpuDevice) -> (WgpuBuffer, wgpu::BindGroup, WgpuPipeline) {
    use molgfx_gpu::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType,
        ComputePipelineDesc, ShaderModuleDesc, ShaderStages,
    };
    let work = device
        .create_buffer(&BufferDesc {
            label: "timestamp compute output",
            size: 16384,
            usage: BufferUsage::STORAGE.union(BufferUsage::COPY_SRC),
        })
        .expect("work buffer allocates");
    let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
        label: "timestamp compute layout",
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Storage { read_only: false },
        }],
    });
    let group = device.create_bind_group(&BindGroupDesc {
        label: "timestamp compute group",
        layout: &layout,
        entries: &[BindGroupEntry::Buffer {
            binding: 0,
            buffer: &work,
        }],
    });
    let shader = device.create_shader_module(&ShaderModuleDesc { label: "timestamp compute", wgsl: "@group(0) @binding(0) var<storage, read_write> output: array<u32>; @compute @workgroup_size(64) fn main(@builtin(global_invocation_id) id: vec3<u32>) { output[id.x] = id.x * 3u + 1u; }" }).expect("compute shader compiles");
    let pipeline = device
        .create_compute_pipeline(&ComputePipelineDesc {
            label: "timestamp compute",
            layouts: &[Some(&layout)],
            shader: &shader,
            entry: "main",
        })
        .expect("compute pipeline creates");
    (work, group, pipeline)
}

fn record_timestamp_work(
    encoder: &mut WgpuCommandEncoder,
    desc: &ComputePassDesc<'_, WgpuDevice>,
    pipeline: &WgpuPipeline,
    group: &wgpu::BindGroup,
) {
    use molgfx_gpu::ComputePassEncoder as _;
    let mut pass = encoder.begin_compute_pass(desc);
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.dispatch(64, 1, 1);
}
