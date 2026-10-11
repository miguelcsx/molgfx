//! Host-owned GPU targets, proven on a real device the test creates itself.
//! Every test skips (passes with a note) when the machine has no adapter.
//! Pixels are read back here, in the test only; the library never reads back.

use molgfx::host::wgpu;
use molgfx::host::{HostContext, HostOutputFormat, host_device_descriptor};
use molgfx::{Renderer, Scene, molframe, profile, rep};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn adapter() -> Option<wgpu::Adapter> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let request = |fallback| {
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter: fallback,
            ..Default::default()
        }))
        .ok()
    };
    let found = request(false).or_else(|| request(true));
    if found.is_none() {
        eprintln!("skipping: no wgpu adapter on this machine");
    }
    found
}

fn context_with(descriptor: &wgpu::DeviceDescriptor<'_>) -> Option<HostContext> {
    let adapter = adapter()?;
    let (device, queue) = pollster::block_on(adapter.request_device(descriptor)).ok()?;
    Some(HostContext {
        adapter,
        device,
        queue,
    })
}

fn host_context() -> Option<HostContext> {
    let adapter = adapter()?;
    let descriptor = host_device_descriptor(&adapter);
    context_with(&descriptor)
}

fn scene() -> Scene {
    const PDB: &str = "\
ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N
ATOM      2  CA  ALA A   1      11.639   6.071  -5.147  1.00  0.00           C
ATOM      3  C   ALA A   1      13.150   6.071  -5.139  1.00  0.00           C
END
";
    let (structure, _) = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("host.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap_or_else(|error| panic!("fixture must parse: {error}"));
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::spacefill("all"))
        .unwrap_or_else(|error| panic!("{error}"));
    scene
}

fn renderer(context: &HostContext) -> Renderer {
    Renderer::from_host(
        profile::converged(),
        context.clone(),
        HostOutputFormat::Rgba8,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

fn host_texture(
    device: &wgpu::Device,
    size: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    view_formats: &[wgpu::TextureFormat],
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("host"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats,
    })
}

fn attachment() -> wgpu::TextureUsages {
    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
}

/// Reads RGBA8 rows back tightly packed. Test-only.
fn read_back(context: &HostContext, texture: &wgpu::Texture) -> Vec<u8> {
    let (width, height) = (texture.width(), texture.height());
    let padded = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        texture.size(),
    );
    let _ = context.queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = context.device.poll(wgpu::PollType::wait_indefinitely());
    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .unwrap_or_else(|error| panic!("{error}"));
    let mut out = Vec::new();
    for row in mapped.chunks(padded as usize) {
        out.extend_from_slice(&row[..(width * 4) as usize]);
    }
    out
}

fn reference(renderer: &mut Renderer, scene: &Scene, camera: &molgfx::Camera) -> Vec<u8> {
    renderer
        .render_image_with_camera(scene, camera, (SIZE, SIZE))
        .unwrap_or_else(|error| panic!("{error}"))
        .pixels()
        .to_vec()
}

#[test]
fn a_host_texture_matches_the_readback_image_without_any_copy() {
    let Some(context) = host_context() else {
        return;
    };
    let (scene, mut renderer) = (scene(), renderer(&context));
    let camera = scene.framing_camera(1.0);
    let expected = reference(&mut renderer, &scene, &camera);
    let texture = host_texture(&context.device, SIZE, FORMAT, attachment(), &[]);
    let target = renderer
        .host_target(&texture)
        .unwrap_or_else(|error| panic!("{error}"));
    let frame = renderer
        .render_to_host_converged(&scene, &camera, &target)
        .unwrap_or_else(|error| panic!("{error}"));
    renderer
        .wait_host_fence_blocking(frame.fence)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(renderer.host_fence_completed(frame.fence).unwrap_or(false));
    let pixels = read_back(&context, &texture);
    assert!(
        pixels.chunks(4).any(|p| p != &pixels[..4]),
        "frame is blank"
    );
    assert_eq!(pixels, expected);
}

#[test]
fn an_srgb_host_texture_renders_through_its_linear_view() {
    let Some(context) = host_context() else {
        return;
    };
    let (scene, mut renderer) = (scene(), renderer(&context));
    let camera = scene.framing_camera(1.0);
    let expected = reference(&mut renderer, &scene, &camera);
    let srgb = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = host_texture(&context.device, SIZE, srgb, attachment(), &[FORMAT]);
    let target = renderer
        .host_target(&texture)
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = renderer
        .render_to_host_converged(&scene, &camera, &target)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(read_back(&context, &texture), expected);
}

#[test]
fn a_texture_the_renderer_cannot_draw_into_is_rejected_with_every_reason() {
    let Some(context) = host_context() else {
        return;
    };
    let renderer = renderer(&context);
    let plain = wgpu::TextureUsages::TEXTURE_BINDING;
    let wrong = host_texture(
        &context.device,
        SIZE,
        wgpu::TextureFormat::Rgba16Float,
        plain,
        &[],
    );
    let message = match renderer.host_target(&wrong) {
        Ok(_) => panic!("texture must be rejected"),
        Err(error) => error.to_string(),
    };
    assert!(message.contains("RENDER_ATTACHMENT"), "{message}");
    assert!(message.contains("Rgba16Float"), "{message}");
    let srgb = wgpu::TextureFormat::Rgba8UnormSrgb;
    let unlisted = host_texture(&context.device, SIZE, srgb, attachment(), &[]);
    let message = match renderer.host_target(&unlisted) {
        Ok(_) => panic!("unlisted view format must be rejected"),
        Err(error) => error.to_string(),
    };
    assert!(message.contains("view_formats"), "{message}");
}

#[test]
fn a_device_below_the_requirements_is_refused_with_the_missing_limits_listed() {
    let Some(adapter) = adapter() else { return };
    let descriptor = wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        ..Default::default()
    };
    let Some(context) = context_with(&descriptor) else {
        return;
    };
    let result = Renderer::from_host(profile::converged(), context, HostOutputFormat::Rgba8);
    let message = match result {
        Ok(_) => panic!("downlevel limits must be refused"),
        Err(error) => error.to_string(),
    };
    assert!(
        message.contains("max_storage_buffers_per_shader_stage"),
        "{message}"
    );
}

#[test]
fn an_owned_target_resizes_by_replacement_and_keeps_rendering() {
    let Some(context) = host_context() else {
        return;
    };
    let (scene, mut renderer) = (scene(), renderer(&context));
    let camera = scene.framing_camera(1.0);
    let small = renderer
        .create_host_target(SIZE, SIZE)
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = renderer
        .render_to_host(&scene, &camera, &small)
        .unwrap_or_else(|error| panic!("{error}"));
    let large = renderer
        .create_host_target(SIZE * 2, SIZE)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!((large.width(), large.height()), (SIZE * 2, SIZE));
    let frame = renderer
        .render_to_host(&scene, &camera, &large)
        .unwrap_or_else(|error| panic!("{error}"));
    renderer
        .wait_host_fence_blocking(frame.fence)
        .unwrap_or_else(|error| panic!("{error}"));
    let pixels = read_back(&context, &large.texture().clone());
    assert_eq!(pixels.len(), (SIZE * 2 * SIZE * 4) as usize);
    assert!(renderer.create_host_target(0, SIZE).is_err());
}

#[test]
fn two_targets_from_one_renderer_each_receive_the_frame() {
    let Some(context) = host_context() else {
        return;
    };
    let (scene, mut renderer) = (scene(), renderer(&context));
    let camera = scene.framing_camera(1.0);
    let expected = reference(&mut renderer, &scene, &camera);
    let first = host_texture(&context.device, SIZE, FORMAT, attachment(), &[]);
    let second = host_texture(&context.device, SIZE, FORMAT, attachment(), &[]);
    for texture in [&first, &second] {
        let target = renderer
            .host_target(texture)
            .unwrap_or_else(|error| panic!("{error}"));
        let _ = renderer
            .render_to_host_converged(&scene, &camera, &target)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    assert_eq!(read_back(&context, &first), expected);
    assert_eq!(read_back(&context, &second), expected);
}

#[test]
fn picking_still_answers_after_rendering_into_a_host_target() {
    let Some(context) = host_context() else {
        return;
    };
    let (scene, mut renderer) = (scene(), renderer(&context));
    let camera = scene.framing_camera(1.0);
    let target = renderer
        .create_host_target(SIZE, SIZE)
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = renderer
        .render_to_host(&scene, &camera, &target)
        .unwrap_or_else(|error| panic!("{error}"));
    let picked = renderer.pick(SIZE / 2, SIZE / 2);
    assert!(picked.is_ok(), "{picked:?}");
}

#[test]
fn host_frames_feed_the_adaptive_loop_so_quality_can_rise() {
    let Some(context) = host_context() else {
        return;
    };
    let scene = scene();
    // An adaptive renderer starts at the reduced tier and earns richer ones from
    // frame-time headroom. Host frames never fed that loop, so a tiny scene
    // rendered at reduced detail for the whole session.
    let mut renderer = Renderer::from_host(
        profile::interactive(),
        context.clone(),
        HostOutputFormat::Rgba8,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let camera = scene.framing_camera(1.0);
    let target = renderer
        .create_host_target(SIZE, SIZE)
        .unwrap_or_else(|error| panic!("{error}"));
    let first = renderer
        .render_to_host(&scene, &camera, &target)
        .unwrap_or_else(|error| panic!("{error}"))
        .report
        .quality_tier;
    let mut last = first;
    for _ in 0..400 {
        let frame = renderer
            .render_to_host(&scene, &camera, &target)
            .unwrap_or_else(|error| panic!("{error}"));
        renderer
            .wait_host_fence_blocking(frame.fence)
            .unwrap_or_else(|error| panic!("{error}"));
        last = frame.report.quality_tier;
    }
    assert!(
        last > first,
        "quality stayed at {first:?} after 400 fast frames"
    );
}
