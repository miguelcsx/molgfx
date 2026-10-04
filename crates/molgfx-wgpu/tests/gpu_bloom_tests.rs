//! Real-device bright-pass regression for narrow HDR highlights.
#![cfg(not(target_arch = "wasm32"))]
use wgpu::util::DeviceExt;

fn uniforms() -> Vec<u8> {
    let module = naga::front::wgsl::parse_str(molgfx_shaders::BLOOM).unwrap();
    let (members, span) = module
        .types
        .iter()
        .find_map(|(_, ty)| match &ty.inner {
            naga::TypeInner::Struct { members, span }
                if ty.name.as_deref() == Some("FrameUniforms") =>
            {
                Some((members, *span))
            }
            _ => None,
        })
        .unwrap();
    let offset = members
        .iter()
        .find(|m| m.name.as_deref() == Some("atmosphere"))
        .unwrap()
        .offset;
    let mut bytes = vec![0; usize::try_from(span).unwrap()];
    let start = usize::try_from(offset).unwrap() + 5 * 16;
    bytes[start..start + 4].copy_from_slice(&1.0_f32.to_ne_bytes());
    bytes
}

fn pipeline(device: &wgpu::Device) -> wgpu::ComputePipeline {
    // Invoke the production fragment body from a deterministic compute probe.
    let body = molgfx_shaders::BLOOM
        .replace("@fragment\nfn fs_bloom_bright", "fn fs_bloom_bright")
        .replacen(") -> @location(0) vec4f {", ") -> vec4f {", 1);
    let source = format!(
        "{body}\n@group(3) @binding(0) var<storage, read_write> result: vec4f;\n@compute @workgroup_size(1) fn probe() {{ result = fs_bloom_bright(FullscreenOut(vec4f(0.5, 0.5, 0.0, 1.0), vec2f(0.5))); }}"
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("production bright pass"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: Some("probe"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn source(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
) -> (wgpu::Texture, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let source_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(1),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        }],
    });
    (texture, source_group)
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn a_single_bright_pixel_survives_bloom_downsampling() {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let pipeline = pipeline(&device);
    let frame = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: &uniforms(),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: frame.as_entire_binding(),
        }],
    });
    let (texture, source_group) = source(&device, &pipeline);
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let results = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(3),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: output.as_entire_binding(),
        }],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    for brightness in [0.5_f32, 4.0] {
        let mut values = vec![0_u8; 256];
        for channel in 0..3 {
            values[channel * 4..channel * 4 + 4].copy_from_slice(&brightness.to_ne_bytes());
        }
        queue.write_texture(
            texture.as_image_copy(),
            &values,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(64),
                rows_per_image: Some(4),
            },
            texture.size(),
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &frame_group, &[]);
            pass.set_bind_group(1, &source_group, &[]);
            pass.set_bind_group(3, &results, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, 16);
        queue.submit([encoder.finish()]);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, |result| result.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        let actual = f32::from_ne_bytes(mapped[..4].try_into().unwrap());
        let expected = if brightness > 1.0 {
            (brightness - 1.0) / 16.0
        } else {
            0.0
        };
        assert!(
            (actual - expected).abs() < 0.0001,
            "bright energy {actual}, expected {expected}"
        );
        drop(mapped);
        readback.unmap();
    }
}
