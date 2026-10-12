//! Close-up impostor bounds must retain visible geometry without full-screen overdraw.
#![cfg(not(target_arch = "wasm32"))]
use wgpu::util::DeviceExt;

fn near_pipeline(device: &wgpu::Device) -> wgpu::ComputePipeline {
    let source = format!(
        "{}
        @group(0) @binding(0) var<uniform> projection: mat4x4f;
        @group(0) @binding(1) var<storage, read_write> result: array<vec4f>;
        fn camera_near_z() -> f32 {{ return -1.0; }}
        fn camera_view_clip(point: vec3f) -> vec4f {{
            return projection * vec4f(point, 1.0);
        }}
        @compute @workgroup_size(1) fn probe() {{
            let minima = array<vec3f, 4>(
                vec3f(-0.2, -0.3, -2.0), vec3f(5.0, -0.3, -2.0),
                vec3f(-6.0, -0.3, -2.0), vec3f(-0.2, -0.3, -0.5));
            let maxima = array<vec3f, 4>(
                vec3f(0.2, 0.3, 0.0), vec3f(6.0, 0.3, 0.0),
                vec3f(-5.0, 0.3, 0.0), vec3f(0.2, 0.3, 0.0));
            for (var i = 0u; i < 4u; i += 1u) {{
                result[i] = vec4f(near_proxy_ndc(minima[i], maxima[i], vec2f(0.0)),
                    near_proxy_ndc(minima[i], maxima[i], vec2f(1.0)));
            }}
        }}",
        include_str!("../../molgfx-shaders/src/wgsl/include/near_proxy.wgsl"),
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("near proxy bounds"),
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

#[test]
#[ignore = "requires a native GPU adapter"]
fn near_crossing_boxes_keep_their_screen_bounds_and_offscreen_boxes_collapse() {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let pipeline = near_pipeline(&device);
    let projection = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let output = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: &[0; 64],
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: projection.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    // Perspective, shifted perspective, and orthographic projections.
    for (perspective, shift) in [(true, 0.0_f32), (true, 0.1), (false, 0.0)] {
        let matrix: [f32; 16] = [
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            -shift,
            0.0,
            0.0,
            if perspective { -1.0 } else { 0.0 },
            0.0,
            0.0,
            0.0,
            if perspective { 0.0 } else { 1.0 },
        ];
        let bytes: Vec<u8> = matrix
            .iter()
            .flat_map(|value| value.to_ne_bytes())
            .collect();
        queue.write_buffer(&projection, 0, &bytes);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, 64);
        queue.submit([encoder.finish()]);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, |result| result.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let mapped = readback.slice(..).get_mapped_range().unwrap();
        let values: [f32; 16] = std::array::from_fn(|index| {
            f32::from_ne_bytes(mapped[index * 4..index * 4 + 4].try_into().unwrap())
        });
        let expected = [-0.2 + shift, -0.3, 0.2 + shift, 0.3];
        for (actual, expected) in values[..4].iter().zip(expected) {
            assert!((actual - expected).abs() < 0.00001, "{values:?}");
        }
        assert_eq!(
            values[4].to_bits(),
            values[6].to_bits(),
            "right offscreen box has zero width"
        );
        assert_eq!(
            values[8].to_bits(),
            values[10].to_bits(),
            "left offscreen box has zero width"
        );
        assert_eq!(&values[12..16], &[0.0; 4], "box before near is empty");
        drop(mapped);
        readback.unmap();
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn near_crossing_spheres_and_capsules_keep_visible_shells_without_offscreen_artifacts() {
    use molgfx_core::{AtomSelection, RepresentationKind, Scene};
    use molgfx_math::{Camera, Projection, Vec3};
    use molgfx_render::{
        AdaptiveQualityConfig, BackdropStyle, Engine, EngineConfig, ImageConfig,
        PresentationEffect, RenderProfile,
    };
    use molgfx_wgpu::WgpuDevice;

    let mut engine = Engine::<WgpuDevice>::new(
        &EngineConfig {
            adaptive: AdaptiveQualityConfig::highest_fixed(60),
            profile: RenderProfile::bare()
                .with_effect(PresentationEffect::Backdrop(BackdropStyle::transparent())),
            ..EngineConfig::default()
        },
        None,
    )
    .unwrap();
    let camera = Camera {
        eye: Vec3::ZERO,
        target: Vec3::new(0.0, 0.0, -3.0),
        up: Vec3::Y,
        projection: Projection::Orthographic {
            height: 8.0,
            aspect: 1.0,
            near: 1.0,
            far: 10.0,
        },
    };
    let visible = "ATOM      1  C   GLY A   1       1.500   0.000  -0.900  1.00 10.00           C  \nATOM      2  O   GLY A   1       2.700   0.000  -0.900  1.00 10.00           O  \n";
    let offscreen = "ATOM      3  C   GLY B   2      10.000   0.000  -0.900  1.00 10.00           C  \nATOM      4  O   GLY B   2      11.200   0.000  -0.900  1.00 10.00           O  \n";
    for form in [RepresentationKind::Spacefill, RepresentationKind::Licorice] {
        let mut images = Vec::new();
        for extra in ["", offscreen] {
            let bytes = format!("{visible}{extra}END\n");
            let (structure, _) = molframe::read_bytes(
                bytes.into_bytes(),
                Some("near.pdb"),
                &molframe::ReadOptions::new(),
            )
            .unwrap();
            let mut scene = Scene::from_structure(&structure).unwrap();
            let selection = scene.add_selection(AtomSelection::All);
            scene.represent(selection, form).unwrap();
            let image = engine
                .render_image(
                    &scene,
                    &camera,
                    ImageConfig {
                        width: 128,
                        height: 128,
                    },
                )
                .unwrap();
            assert!(
                image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[3] > 0),
                "{form:?} must retain its shell behind the near plane"
            );
            let coverage: Vec<u8> = image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|pixel| pixel[3])
                .collect();
            images.push(coverage);
        }
        assert!(
            images[0] == images[1],
            "offscreen {form:?} changes visible pixels"
        );
    }
}
