//! Drawn-geometry ray bounds preserve small and enlarged spheres.
#![cfg(not(target_arch = "wasm32"))]

fn source() -> String {
    format!(
        "struct BvhNode {{ min_left: vec4f, max_radius: vec4f }}
        const BVH_COUNT_SHIFT: u32 = 29u;
        const BVH_INDEX_MASK: u32 = 0x1fffffffu;
        {}
        {}
        fn ray_capsule(d: vec3f, a: vec3f, b: vec3f, r: f32) -> f32 {{ return -1.0; }}
        fn box_interval(o: vec3f, d: vec3f, lo: vec3f, hi: vec3f) -> vec2f {{
            let a = (lo - o) * d;
            let b = (hi - o) * d;
            let near = min(a, b);
            let far = max(a, b);
            return vec2f(max(max(near.x, near.y), near.z), min(min(far.x, far.y), far.z));
        }}
        @group(0) @binding(0) var<storage, read_write> result: array<vec4u>;
        @compute @workgroup_size(64) fn probe(@builtin(global_invocation_id) id: vec3u) {{
            let index = id.x % 1024u;
            let enlarged = id.x >= 1024u;
            let radius = select(0.25, 4.3, enlarged);
            let origin = vec3f(-10.0, (f32(index % 32u) - 15.5) / 3.0,
                (f32(index / 32u) - 15.5) / 3.0);
            let direction = normalize(vec3f(1.0, 0.01, 0.02));
            let node = BvhNode(vec4f(vec3f(-1.7), 0.0), vec4f(vec3f(1.7), 1.7));
            let drawn = BvhNode(vec4f(vec3f(-radius), 0.0), vec4f(vec3f(radius), radius));
            let tight = quality_node_interval(origin, 1.0 / direction, drawn, 0.0);
            let old = quality_node_interval(origin, 1.0 / direction, node, node.max_radius.w);
            let distance = quality_sphere_distance(origin, direction, vec3f(0.0), radius);
            result[id.x] = vec4u(u32(distance > 0.0 && distance < 20.0),
                u32(quality_node_is_missed(tight, 20.0)),
                u32(quality_node_is_missed(old, 20.0)), u32(enlarged));
        }}",
        include_str!("../../molgfx-shaders/src/wgsl/include/quality/analytic.wgsl"),
        include_str!("../../molgfx-shaders/src/wgsl/include/quality/traversal.wgsl"),
    )
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn tight_ray_bounds_keep_every_sphere_hit_and_reject_more_empty_space() {
    let mapped = run(&source());
    let rows: Vec<[u32; 4]> = mapped
        .as_chunks::<16>()
        .0
        .iter()
        .map(|bytes| {
            std::array::from_fn(|i| u32::from_ne_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()))
        })
        .collect();
    assert!(rows.iter().any(|row| row[0] != 0 && row[3] == 0));
    assert!(rows.iter().any(|row| row[0] != 0 && row[3] == 1));
    assert!(
        rows.iter().all(|row| row[0] == 0 || row[1] == 0),
        "a sphere hit was culled"
    );
    assert!(
        rows.iter()
            .any(|row| row[3] == 0 && row[1] == 1 && row[2] == 0),
        "small spheres must reject more empty space"
    );
    assert!(
        rows.iter()
            .any(|row| row[3] == 1 && row[0] == 1 && row[2] == 1),
        "enlarged spheres need their additional radius preserved"
    );
}

fn run(shader: &str) -> Vec<u8> {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("quality traversal probe"),
        source: wgpu::ShaderSource::Wgsl(shader.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: Some("probe"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 2048 * 16,
        mapped_at_creation: false,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: output.as_entire_binding(),
        }],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: output.size(),
        mapped_at_creation: false,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output.size());
    queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| result.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    readback.slice(..).get_mapped_range().unwrap().to_vec()
}
