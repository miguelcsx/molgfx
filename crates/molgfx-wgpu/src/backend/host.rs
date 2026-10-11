//! Host-supplied wgpu contexts and textures: the sanctioned place where the
//! backend's native types cross the public boundary.
//!
//! An embedding application that already owns a wgpu adapter, device and
//! queue lends them to the renderer here. The module reports what the host
//! must enable when it creates that device, validates the device it is
//! given against the same requirements, and checks each texture the host
//! asks the renderer to draw into.

use super::convert;
use super::device::WgpuDevice;
use super::device_caps::REQUIRED_STORAGE_BUFFERS_PER_STAGE;
use super::device_errors::DeviceErrors;
use super::queue::WgpuQueue;
use super::resource::{ResourceLedger, WgpuTexture};
use molgfx_gpu::{GpuError, HostDevice, HostDeviceDesc, ImportedTarget, Opened, TextureFormat};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

/// The wgpu handles a host lends to the renderer. All three must come from
/// one adapter; each is a cheap shared handle, so the host keeps its own
/// clones and both sides submit to the same queue.
#[derive(Clone, Debug)]
pub struct HostContext {
    /// The adapter the device was requested from.
    pub adapter: wgpu::Adapter,
    /// The host's device.
    pub device: wgpu::Device,
    /// The queue every submission, the renderer's and the host's, goes
    /// through. One queue means one submission order.
    pub queue: wgpu::Queue,
}

/// What a device must provide for the renderer to run on it.
#[derive(Clone, Debug)]
pub struct HostRequirements {
    /// Features the device must have been created with. Empty today: the
    /// renderer needs none beyond the baseline.
    pub features: wgpu::Features,
    /// Features the renderer uses when present and does without otherwise.
    /// `TIMESTAMP_QUERY` enables per-pass GPU timings.
    pub optional_features: wgpu::Features,
    /// The floor for every limit. The default WebGPU limits, which already
    /// include eight storage buffers per shader stage; `downlevel` limit sets
    /// do not satisfy it. Hosts should request more where the adapter offers
    /// it: [`host_device_descriptor`] raises the buffer-size limits to the
    /// adapter's maximum so large structures fit.
    pub limits: wgpu::Limits,
    /// Formats the adapter must allow as write-only storage textures.
    pub storage_texture_formats: &'static [wgpu::TextureFormat],
}

/// The requirements a host device must meet, to request when creating it.
#[must_use]
pub fn host_requirements() -> HostRequirements {
    HostRequirements {
        features: wgpu::Features::empty(),
        optional_features: wgpu::Features::TIMESTAMP_QUERY,
        limits: wgpu::Limits {
            max_storage_buffers_per_shader_stage: REQUIRED_STORAGE_BUFFERS_PER_STAGE,
            ..wgpu::Limits::default()
        },
        storage_texture_formats: &[wgpu::TextureFormat::R32Float],
    }
}

/// A device descriptor satisfying [`host_requirements`] on `adapter`, with
/// the optional features the adapter supports and its maximum buffer sizes.
/// A host with needs of its own should merge these into its descriptor.
#[must_use]
pub fn host_device_descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    let supported = adapter.limits();
    wgpu::DeviceDescriptor {
        label: Some("molgfx host"),
        required_features: WgpuDevice::negotiated_features(adapter.features()),
        required_limits: WgpuDevice::required_limits(&supported, false),
        ..Default::default()
    }
}

/// Every requirement the context fails, one sentence each; empty when the
/// context can run the renderer.
#[must_use]
pub fn unmet_requirements(context: &HostContext) -> Vec<String> {
    let required = host_requirements();
    let mut missing = Vec::new();
    let features = context.device.features();
    for (name, _) in required.features.difference(features).iter_names() {
        missing.push(format!("device feature {name} was not enabled"));
    }
    required.limits.check_limits_with_fail_fn(
        &context.device.limits(),
        false,
        |name, needed, offered| {
            missing.push(format!(
                "device limit {name} is {offered}, the renderer needs {needed}"
            ));
        },
    );
    for format in required.storage_texture_formats {
        let allowed = context
            .adapter
            .get_texture_format_features(*format)
            .allowed_usages;
        if !allowed.contains(wgpu::TextureUsages::STORAGE_BINDING) {
            missing.push(format!("adapter cannot write {format:?} storage textures"));
        }
    }
    missing
}

impl HostDevice for WgpuDevice {
    type Context = HostContext;
    type ExternalTexture = wgpu::Texture;

    fn adopt(context: HostContext, desc: &HostDeviceDesc) -> Result<Opened<Self>, GpuError> {
        let missing = unmet_requirements(&context);
        if !missing.is_empty() {
            return Err(GpuError::HostRequirements { missing });
        }
        let HostContext {
            adapter,
            device,
            queue,
        } = context;
        let capabilities =
            Self::probe_adapter_capabilities(&adapter, device.features(), &device.limits());
        let errors = if desc.observe_device_errors {
            DeviceErrors::attach(&device)
        } else {
            Arc::default()
        };
        Ok(Opened {
            device: Self {
                device,
                capabilities,
                resources: Arc::new(ResourceLedger::new(desc.resource_memory_limit_bytes)),
                errors,
            },
            queue: WgpuQueue {
                queue,
                next_fence: Arc::new(AtomicU64::new(1)),
                completed_fence: Arc::new(AtomicU64::new(0)),
            },
            surface: None,
        })
    }

    fn import_target(
        &self,
        texture: &wgpu::Texture,
        render_format: TextureFormat,
    ) -> Result<(wgpu::TextureView, ImportedTarget), GpuError> {
        let render = convert::texture_format(render_format);
        let own = texture.format();
        let problems = target_problems(texture, render, &self.device.limits());
        if !problems.is_empty() {
            return Err(GpuError::HostTarget { problems });
        }
        let view = if own == render {
            texture.create_view(&wgpu::TextureViewDescriptor::default())
        } else {
            // The host must have listed the linear format in `view_formats`;
            // wgpu reports it only when the view is made.
            self.view_checked(texture, render)?
        };
        let texture_format = match convert::surface_format_back(own) {
            Some(format) => format,
            None => render_format,
        };
        Ok((
            view,
            ImportedTarget {
                width: texture.width(),
                height: texture.height(),
                texture_format,
                render_format,
            },
        ))
    }

    fn export_texture(&self, texture: &WgpuTexture) -> wgpu::Texture {
        texture.raw.clone()
    }
}

fn target_problems(
    texture: &wgpu::Texture,
    render: wgpu::TextureFormat,
    limits: &wgpu::Limits,
) -> Vec<String> {
    let mut problems = Vec::new();
    let own = texture.format();
    if own.remove_srgb_suffix() != render {
        problems.push(format!(
            "format {own:?} does not belong to the {render:?} family the renderer writes"
        ));
    }
    if !texture
        .usage()
        .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    {
        problems.push("usage lacks RENDER_ATTACHMENT".to_owned());
    }
    if texture.dimension() != wgpu::TextureDimension::D2 {
        problems.push("texture is not 2-D".to_owned());
    }
    if texture.depth_or_array_layers() != 1 {
        problems.push("texture has more than one layer".to_owned());
    }
    if texture.mip_level_count() != 1 {
        problems.push("texture has more than one mip level".to_owned());
    }
    if texture.sample_count() != 1 {
        problems.push("texture is multisampled".to_owned());
    }
    let largest = texture.width().max(texture.height());
    if texture.width() == 0 || texture.height() == 0 || largest > limits.max_texture_dimension_2d {
        problems.push(format!(
            "extent {}x{} is outside 1..={}",
            texture.width(),
            texture.height(),
            limits.max_texture_dimension_2d
        ));
    }
    problems
}

impl WgpuDevice {
    /// A view in `format`, reporting a validation failure (the host did not
    /// list the format in the texture's `view_formats`) as a typed error.
    #[cfg(not(target_arch = "wasm32"))]
    fn view_checked(
        &self,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
    ) -> Result<wgpu::TextureView, GpuError> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format),
            ..Default::default()
        });
        let error: Option<wgpu::Error> = pollster::block_on(scope.pop());
        match error {
            None => Ok(view),
            Some(error) => Err(GpuError::HostTarget {
                problems: vec![format!(
                    "the texture's view_formats must include {format:?}: {error}"
                )],
            }),
        }
    }

    /// Browser validation errors arrive through the device mailbox instead.
    #[cfg(target_arch = "wasm32")]
    fn view_checked(
        &self,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
    ) -> Result<wgpu::TextureView, GpuError> {
        Ok(texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format),
            ..Default::default()
        }))
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
