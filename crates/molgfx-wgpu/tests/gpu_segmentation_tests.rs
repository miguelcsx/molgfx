//! Exact categorical-volume integration on a real GPU.

#![cfg(not(target_arch = "wasm32"))]

use wgpu::util::DeviceExt;

const PROBE: &str = r"
@group(3) @binding(0) var<storage, read_write> probes: array<vec4f>;
@compute @workgroup_size(1)
fn probe_segments() {
    for (var index = 0u; index < 2u; index++) {
        let origin = vec3f(select(0.0, 1000.0, index == 1u), 0.0, 0.0);
        let direction = vec3f(select(1.0, -1.0, index == 1u), 0.0, 0.0);
        let scale = volume.sampling.z;
        let ray = SegmentationRay(origin * scale, direction, origin, direction / scale,
            vec3f(direction.x * scale, SEGMENT_INFINITY, SEGMENT_INFINITY), 0.0, 0.0);
        let result = integrate_segments(ray, vec2f(0.0, 1000.0 * scale));
        probes[index] = vec4f(result.accumulated.a, result.representative_t,
            bitcast<f32>(result.label), result.accumulated.r);
    }
    probes[2] = vec4f(segment_local_normal(vec3f(899.5, 0.0, 0.0), 7u), 0.0);
    probes[3] = vec4f(segment_local_normal(vec3f(900.5, 0.0, 0.0), 7u), 0.0);
}
";

fn uniforms(scale: f32) -> Vec<u8> {
    let module = naga::front::wgsl::parse_str(molgfx_shaders::SEGMENTATION).unwrap();
    let (members, span) = module
        .types
        .iter()
        .find_map(|(_, ty)| match &ty.inner {
            naga::TypeInner::Struct { members, span }
                if ty.name.as_deref() == Some("SegmentationUniforms") =>
            {
                Some((members, *span))
            }
            _ => None,
        })
        .unwrap();
    let mut bytes = vec![0; usize::try_from(span).unwrap()];
    for (name, values) in [
        ("dimensions", [1001_u32, 2, 2, 0]),
        (
            "sampling",
            [1.0_f32.to_bits(), 0.65_f32.to_bits(), scale.to_bits(), 0],
        ),
        ("lookup", [0, 8, 7, 1]),
    ] {
        let member = members
            .iter()
            .find(|member| member.name.as_deref() == Some(name))
            .unwrap();
        let start = usize::try_from(member.offset).unwrap();
        for (index, value) in values.into_iter().enumerate() {
            bytes[start + index * 4..start + (index + 1) * 4].copy_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

fn field(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::ComputePipeline,
    scale: f32,
) -> wgpu::BindGroup {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("thin distant segment"),
        size: wgpu::Extent3d {
            width: 1001,
            height: 2,
            depth_or_array_layers: 2,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut labels = vec![0_u32; 1001 * 4];
    for row in 0..4 {
        labels[row * 1001 + 900] = 7;
    }
    queue.write_texture(
        texture.as_image_copy(),
        &labels
            .iter()
            .flat_map(|value| value.to_ne_bytes())
            .collect::<Vec<_>>(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4004),
            rows_per_image: Some(2),
        },
        texture.size(),
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let volume = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("segment addressing"),
        contents: &uniforms(scale),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let mut styles = vec![0_u32; 8 * 8];
    styles[7 * 8..8 * 8].copy_from_slice(&[
        7,
        1,
        0.01_f32.to_bits(),
        0,
        (128.0_f32 / 255.0).to_bits(),
        0,
        0,
        1.0_f32.to_bits(),
    ]);
    let styles = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("faint label style"),
        contents: &styles
            .iter()
            .flat_map(|value| value.to_ne_bytes())
            .collect::<Vec<_>>(),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let dummy = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("unused sparse segment bindings"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("categorical field"),
        layout: &pipeline.get_bind_group_layout(2),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: volume.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: styles.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: dummy.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: dummy.as_entire_binding(),
            },
        ],
    })
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn faint_thin_segments_keep_their_identity_and_integrate_equally_in_both_directions() {
    verify_integration(1.0);
    verify_integration(1e-6);
}

fn verify_integration(scale: f32) {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("segment integration regression"),
        source: wgpu::ShaderSource::Wgsl(format!("{}{PROBE}", molgfx_shaders::SEGMENTATION).into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("segment probe"),
        layout: None,
        module: &shader,
        entry_point: Some("probe_segments"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let field = field(&device, &queue, &pipeline, scale);
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("integration results"),
        size: 64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let results = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("probe results"),
        layout: &pipeline.get_bind_group_layout(3),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: output.as_entire_binding(),
        }],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("integration readback"),
        size: 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(2, &field, &[]);
        pass.set_bind_group(3, &results, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, 64);
    queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| result.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = readback.slice(..).get_mapped_range().unwrap();
    let values: Vec<f32> = mapped
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_ne_bytes(*bytes))
        .collect();
    let expected_alpha = 1.0 - (-0.01_f32).exp();
    for result in values[..8].as_chunks::<4>().0 {
        assert!((result[0] - expected_alpha).abs() < 0.00001, "{result:?}");
        assert_eq!(
            result[2].to_bits(),
            7,
            "a visible region must never retain label zero"
        );
        let expected_red = ((128.0_f32 / 255.0 + 0.055) / 1.055).powf(2.4);
        assert!((result[3] - expected_alpha * expected_red).abs() < 0.00001);
    }
    assert!(
        (values[1] / scale - 899.5).abs() < 0.001,
        "scale={scale} values={values:?}"
    );
    assert!((values[5] / scale - 99.5).abs() < 0.001);
    assert_eq!(&values[8..11], &[-0.5, 0.0, 0.0]);
    assert_eq!(&values[12..15], &[0.5, 0.0, 0.0]);
}
