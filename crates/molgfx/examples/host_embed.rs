//! Embedding `MolGFX` in an application that already owns a wgpu device.
//!
//! Run with `cargo run -p molgfx --features host --example host_embed`.
//! The host creates its device from `host_device_descriptor`, lends it to the
//! renderer, and gets a frame drawn into a texture: no CPU copy.

use molgfx::host::wgpu;
use molgfx::host::{HostContext, HostOutputFormat, host_device_descriptor};
use molgfx::{Renderer, Scene, molframe, profile, rep};

const PDB: &str = "\
ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N
ATOM      2  CA  ALA A   1      11.639   6.071  -5.147  1.00  0.00           C
ATOM      3  C   ALA A   1      13.150   6.071  -5.139  1.00  0.00           C
END
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. The host's own adapter, device and queue. A real host merges the
    //    renderer's descriptor into its own.
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&host_device_descriptor(&adapter)))?;
    let context = HostContext {
        adapter,
        device,
        queue,
    };

    // 2. Lend them to the renderer. Missing features or limits come back as a
    //    typed error that lists each one.
    let mut renderer =
        Renderer::from_host(profile::interactive(), context, HostOutputFormat::Rgba8)?;

    // 3. A scene, as always.
    let (structure, _) = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("host.pdb"),
        &molframe::ReadOptions::new(),
    )?;
    let mut scene = Scene::from_structure(&structure)?;
    let _ = scene.add(rep::spacefill("all"))?;

    // 4. A texture the renderer owns; the host samples it when compositing.
    //    (A host texture goes through `Renderer::host_target` instead.)
    let target = renderer.create_host_target(256, 256)?;
    let camera = scene.framing_camera(1.0);
    let frame = renderer.render_to_host(&scene, &camera, &target)?;

    // 5. The fence is optional. Work the host submits on the same queue after
    //    this point is already ordered after the frame.
    renderer.wait_host_fence_blocking(frame.fence)?;
    println!(
        "rendered {}x{}; needs another frame: {}",
        target.width(),
        target.height(),
        frame.report.needs_another_frame
    );
    Ok(())
}
