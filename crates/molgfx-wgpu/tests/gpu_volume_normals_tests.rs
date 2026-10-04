//! Real-device checks of the scalar field and its shading gradient.

#![cfg(not(target_arch = "wasm32"))]

use wgpu::util::DeviceExt;

const PROBE: &str = r"
@group(3) @binding(0) var<storage, read_write> probes: array<vec4f>;
@compute @workgroup_size(1)
fn probe_normals() {
    let points = array<vec3f, 4>(
        vec3f(1.9999, 1.5, 1.5), vec3f(2.0001, 1.5, 1.5),
        vec3f(0.0, 1.5, 1.5), vec3f(4.0, 1.5, 1.5),
    );
    for (var i = 0u; i < 4u; i++) {
        probes[i] = vec4f(density_shading_gradient(points[i]), density_at(points[i]));
    }
}
";

fn pipeline(device: &wgpu::Device) -> wgpu::ComputePipeline {
    let source = format!("{}{PROBE}", molgfx_shaders::VOLUME);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("volume gradient regression"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("volume gradient probe"),
        layout: None,
        module: &shader,
        entry_point: Some("probe_normals"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn dimensions() -> Vec<u8> {
    let module = naga::front::wgsl::parse_str(molgfx_shaders::VOLUME).unwrap();
    let (members, span) = module
        .types
        .iter()
        .find_map(|(_, ty)| match &ty.inner {
            naga::TypeInner::Struct { members, span }
                if ty.name.as_deref() == Some("VolumeUniforms") =>
            {
                Some((members, *span))
            }
            _ => None,
        })
        .unwrap();
    let dimensions_offset = members
        .iter()
        .find(|member| member.name.as_deref() == Some("dimensions"))
        .unwrap()
        .offset;
    let mut uniforms = vec![0; usize::try_from(span).unwrap()];
    for axis in 0..3 {
        let start = usize::try_from(dimensions_offset).unwrap() + axis * 4;
        uniforms[start..start + 4].copy_from_slice(&5_u32.to_ne_bytes());
    }
    uniforms
}

fn field(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
) -> (wgpu::Texture, wgpu::BindGroup) {
    let volume = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("probe dimensions"),
        contents: &dimensions(),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let density = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("probe scalar field"),
        size: wgpu::Extent3d {
            width: 5,
            height: 5,
            depth_or_array_layers: 5,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = density.create_view(&wgpu::TextureViewDescriptor::default());
    let dummy = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("unused sparse volume bindings"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let field = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("probe field"),
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
                binding: 3,
                resource: dummy.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: dummy.as_entire_binding(),
            },
        ],
    });
    (density, field)
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn lighting_gradients_are_continuous_and_preserve_affine_fields_at_grid_edges() {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let pipeline = pipeline(&device);
    let (density, field) = field(&device, &pipeline);
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gradient results"),
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
        label: Some("gradient readback"),
        size: 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    for quadratic in [false, true] {
        let mut values = Vec::new();
        for z in 0_u16..5 {
            for y in 0_u16..5 {
                for x in 0_u16..5 {
                    let x = f32::from(x);
                    let value = if quadratic { x * x } else { x * 2.0 }
                        + f32::from(y) * 3.0
                        + f32::from(z) * 5.0;
                    values.extend_from_slice(&value.to_ne_bytes());
                }
            }
        }
        queue.write_texture(
            density.as_image_copy(),
            &values,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(20),
                rows_per_image: Some(5),
            },
            density.size(),
        );
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
        let samples: Vec<f32> = mapped
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_ne_bytes(*bytes))
            .collect();
        assert!(
            (samples[0] - samples[4]).abs() < 0.001,
            "cell seam: {samples:?}"
        );
        for point in samples.as_chunks::<4>().0 {
            assert!((point[1] - 3.0).abs() < 0.0001);
            assert!((point[2] - 5.0).abs() < 0.0001);
            if !quadratic {
                assert!(
                    (point[0] - 2.0).abs() < 0.0001,
                    "affine boundary: {point:?}"
                );
            }
        }
        let expected_x = if quadratic { 4.0 } else { 2.0 };
        assert!((samples[0] - expected_x).abs() < 0.001);
        assert!((samples[3] - 16.0).abs() < 0.001, "scalar crossing changed");
        drop(mapped);
        readback.unmap();
    }
}
