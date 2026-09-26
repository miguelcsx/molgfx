//! The wgpu device: adapter selection, resource creation, capabilities.

use crate::convert;
use crate::surface::WgpuSurface;
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BufferDesc, Capabilities,
    ComputePipelineDesc, DeviceDesc, GpuError, Opened, RenderPipelineDesc, SamplerDesc,
    ShaderModuleDesc, TextureDesc, TextureViewDesc, WindowTarget,
};
use std::future::Future;
use std::sync::Arc;

use super::device_errors::DeviceErrors;
use super::resource::{ResourceLedger, WgpuBuffer, WgpuTexture, texture_bytes};

#[path = "device/adapter.rs"]
mod adapter;
use adapter::{opposite_power, request_adapter, select_headless_adapter, wgpu_power};

#[cfg(test)]
use super::device_caps::REQUIRED_STORAGE_BUFFERS_PER_STAGE;

/// The wgpu implementation of the device abstraction.
#[derive(Debug)]
pub struct WgpuDevice {
    pub(crate) device: wgpu::Device,
    pub(super) capabilities: Capabilities,
    pub(crate) resources: Arc<ResourceLedger>,
    pub(super) errors: Arc<DeviceErrors>,
}

impl WgpuDevice {
    fn validate_buffer(desc: &BufferDesc, limits: &wgpu::Limits) -> Result<(), GpuError> {
        let mut limit = limits.max_buffer_size;
        if desc.usage.contains(molgfx_gpu::BufferUsage::STORAGE) {
            limit = limit.min(limits.max_storage_buffer_binding_size);
        }
        if desc.size > limit {
            return Err(GpuError::LimitExceeded {
                resource: desc.label,
                limit,
            });
        }
        Ok(())
    }

    async fn finish_open(
        desc: &DeviceDesc,
        adapter: wgpu::Adapter,
        surface: Option<wgpu::Surface<'static>>,
    ) -> Result<Opened<Self>, GpuError> {
        let features = adapter.features();
        let supported_limits = adapter.limits();
        Self::validate_required_limits(&supported_limits)?;
        let required_features = Self::negotiated_features(features);
        let ray_query = required_features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("molgfx"),
                required_features,
                required_limits: Self::required_limits(&supported_limits, ray_query),
                ..Default::default()
            })
            .await
            .map_err(|error| GpuError::DeviceRequest {
                detail: error.to_string(),
            })?;
        let capabilities =
            Self::probe_adapter_capabilities(&adapter, device.features(), &device.limits());
        let errors = DeviceErrors::attach(&device);
        let surface = surface.map(|surface| {
            let mut surface = WgpuSurface::new(surface, &adapter, &device);
            surface.attach_queue(queue.clone());
            surface
        });
        Ok(Opened {
            device: Self {
                device,
                capabilities,
                resources: Arc::new(ResourceLedger::new(desc.resource_memory_limit_bytes)),
                errors,
            },
            queue: crate::queue::WgpuQueue {
                queue,
                next_fence: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
                completed_fence: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            },
            surface,
        })
    }
}

impl molgfx_gpu::Device for WgpuDevice {
    type Buffer = WgpuBuffer;
    type Texture = WgpuTexture;
    type TextureView = wgpu::TextureView;
    type Sampler = wgpu::Sampler;
    type ShaderModule = wgpu::ShaderModule;
    type BindGroupLayout = wgpu::BindGroupLayout;
    type BindGroup = wgpu::BindGroup;
    type Pipeline = crate::encoder::WgpuPipeline;
    type QuerySet = wgpu::QuerySet;
    type Blas = wgpu::Blas;
    type Tlas = wgpu::Tlas;
    type CommandEncoder = crate::encoder::WgpuCommandEncoder;
    type Queue = crate::queue::WgpuQueue;
    type Surface = WgpuSurface;

    fn open_async(
        desc: &DeviceDesc,
        window: Option<WindowTarget>,
    ) -> impl Future<Output = Result<Opened<Self>, GpuError>> {
        let power = desc.power;
        async move {
            // Passing the display handle lets Wayland and X11 pick the right
            // connection; headless opens need none.
            #[cfg(not(target_arch = "wasm32"))]
            let instance_desc = match &window {
                Some(window) => wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                    Box::new(window.clone()),
                ),
                None => wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            };
            #[cfg(target_arch = "wasm32")]
            let instance_desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
            let instance = wgpu::Instance::new(instance_desc);

            #[cfg(not(target_arch = "wasm32"))]
            let surface = match window {
                Some(window) => {
                    let target = wgpu::SurfaceTarget::Window(Box::new(window));
                    let surface = instance.create_surface(target).map_err(|error| {
                        GpuError::SurfaceCreation {
                            detail: error.to_string(),
                        }
                    })?;
                    Some(surface)
                }
                None => None,
            };
            #[cfg(target_arch = "wasm32")]
            let surface = match window {
                Some(canvas) => Some(
                    instance
                        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                        .map_err(|error| GpuError::SurfaceCreation {
                            detail: error.to_string(),
                        })?,
                ),
                None => None,
            };

            let adapter = if surface.is_none() {
                select_headless_adapter(&instance, power)
                    .await
                    .map_err(|detail| GpuError::NoAdapter { detail })?
            } else {
                let requested_power = wgpu_power(power);
                let requested =
                    request_adapter(&instance, surface.as_ref(), requested_power, false).await;
                match requested {
                    Ok(adapter) => adapter,
                    Err(primary) => {
                        let fallback_power = opposite_power(requested_power);
                        match request_adapter(&instance, surface.as_ref(), fallback_power, false).await {
                            Ok(adapter) => adapter,
                            Err(fallback) => request_adapter(
                                &instance,
                                surface.as_ref(),
                                wgpu::PowerPreference::LowPower,
                                true,
                            )
                            .await
                            .map_err(|software| GpuError::NoAdapter {
                                detail: format!(
                                    "requested {requested_power:?}: {primary}; other power {fallback_power:?}: {fallback}; fallback adapter: {software}. No usable graphics API was found; see molgfx.system_info()"
                                ),
                            })?,
                        }
                    }
                }
            };
            Self::finish_open(desc, adapter, surface).await
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_blocking(
        desc: &DeviceDesc,
        window: Option<WindowTarget>,
    ) -> Result<Opened<Self>, GpuError> {
        pollster::block_on(<Self as molgfx_gpu::Device>::open_async(desc, window))
    }

    fn create_buffer(&self, desc: &BufferDesc) -> Result<WgpuBuffer, GpuError> {
        Self::validate_buffer(desc, &self.device.limits())?;
        self.resources.reserve_buffer(desc.size, desc.label)?;
        let raw = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(desc.label),
            size: desc.size,
            usage: convert::buffer_usage(desc.usage),
            mapped_at_creation: false,
        });
        Ok(WgpuBuffer::new(raw, desc.size, Arc::clone(&self.resources)))
    }

    fn create_texture(&self, desc: &TextureDesc) -> Result<WgpuTexture, GpuError> {
        let bytes = texture_bytes(desc)?;
        self.resources.reserve_texture(bytes, desc.label)?;
        let raw = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(desc.label),
            size: wgpu::Extent3d {
                width: desc.width.max(1),
                height: desc.height.max(1),
                depth_or_array_layers: desc.depth.max(1),
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: convert::texture_dimension(desc.dimension),
            format: convert::texture_format(desc.format),
            usage: convert::texture_usage(desc.usage),
            view_formats: &[],
        });
        Ok(WgpuTexture::new(raw, bytes, Arc::clone(&self.resources)))
    }

    fn create_texture_view(
        &self,
        texture: &WgpuTexture,
        _desc: &TextureViewDesc,
    ) -> wgpu::TextureView {
        texture
            .raw
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn create_sampler(&self, desc: &SamplerDesc) -> wgpu::Sampler {
        let filter = convert::filter_mode(desc.filter);
        self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(desc.label),
            mag_filter: filter,
            min_filter: filter,
            compare: desc.compare.map(convert::compare_function),
            ..Default::default()
        })
    }

    fn create_shader_module(
        &self,
        desc: &ShaderModuleDesc<'_>,
    ) -> Result<wgpu::ShaderModule, GpuError> {
        self.validated(desc.label, || {
            self.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(desc.label),
                    source: wgpu::ShaderSource::Wgsl(desc.wgsl.into()),
                })
        })
    }

    fn create_bind_group_layout(&self, desc: &BindGroupLayoutDesc<'_>) -> wgpu::BindGroupLayout {
        let entries: Vec<wgpu::BindGroupLayoutEntry> = desc
            .entries
            .iter()
            .map(|e| wgpu::BindGroupLayoutEntry {
                binding: e.binding,
                visibility: convert::shader_stages(e.visibility),
                ty: convert::binding_type(e.ty),
                count: None,
            })
            .collect();
        self.device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(desc.label),
                entries: &entries,
            })
    }

    fn create_bind_group(&self, desc: &BindGroupDesc<'_, Self>) -> wgpu::BindGroup {
        let entries: Vec<wgpu::BindGroupEntry<'_>> = desc
            .entries
            .iter()
            .map(|e| match e {
                BindGroupEntry::Buffer { binding, buffer } => wgpu::BindGroupEntry {
                    binding: *binding,
                    resource: buffer.raw.as_entire_binding(),
                },
                BindGroupEntry::BufferRange {
                    binding,
                    buffer,
                    offset,
                    size,
                } => wgpu::BindGroupEntry {
                    binding: *binding,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer.raw,
                        offset: *offset,
                        size: std::num::NonZeroU64::new(*size),
                    }),
                },
                BindGroupEntry::Texture { binding, view } => wgpu::BindGroupEntry {
                    binding: *binding,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                BindGroupEntry::Sampler { binding, sampler } => wgpu::BindGroupEntry {
                    binding: *binding,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            })
            .collect();
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(desc.label),
            layout: desc.layout,
            entries: &entries,
        })
    }

    fn create_render_pipeline(
        &self,
        desc: &RenderPipelineDesc<'_, Self>,
    ) -> Result<crate::encoder::WgpuPipeline, GpuError> {
        let layouts: Vec<Option<&wgpu::BindGroupLayout>> = desc.layouts.to_vec();
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(desc.label),
                bind_group_layouts: &layouts,
                immediate_size: 0,
            });
        let targets: Vec<Option<wgpu::ColorTargetState>> = desc
            .color_targets
            .iter()
            .map(|t| {
                Some(wgpu::ColorTargetState {
                    format: convert::texture_format(t.format),
                    blend: convert::blend_state(t.blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })
            })
            .collect();
        // Both stages share the override values: an entry point that does not
        // declare one simply ignores it, and splitting them would let the
        // vertex and fragment halves of one pipeline specialize differently.
        let compilation_options = wgpu::PipelineCompilationOptions {
            constants: desc.constants,
            ..Default::default()
        };
        self.validated(desc.label, || {
            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(desc.label),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: desc.shader,
                        entry_point: Some(desc.vs_entry),
                        compilation_options: compilation_options.clone(),
                        buffers: &[],
                    },
                    fragment: desc.fs_entry.map(|entry| wgpu::FragmentState {
                        module: desc.shader,
                        entry_point: Some(entry),
                        compilation_options: compilation_options.clone(),
                        targets: &targets,
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: convert::topology(desc.topology),
                        ..Default::default()
                    },
                    depth_stencil: desc.depth.map(|d| wgpu::DepthStencilState {
                        format: convert::texture_format(d.format),
                        depth_write_enabled: Some(d.write),
                        depth_compare: Some(convert::compare_function(d.compare)),
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                })
        })
        .map(crate::encoder::WgpuPipeline::Render)
    }

    fn create_compute_pipeline(
        &self,
        desc: &ComputePipelineDesc<'_, Self>,
    ) -> Result<crate::encoder::WgpuPipeline, GpuError> {
        let layouts: Vec<Option<&wgpu::BindGroupLayout>> = desc.layouts.to_vec();
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(desc.label),
                bind_group_layouts: &layouts,
                immediate_size: 0,
            });
        self.validated(desc.label, || {
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(desc.label),
                    layout: Some(&layout),
                    module: desc.shader,
                    entry_point: Some(desc.entry),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                })
        })
        .map(crate::encoder::WgpuPipeline::Compute)
    }

    fn create_command_encoder(&self) -> crate::encoder::WgpuCommandEncoder {
        crate::encoder::WgpuCommandEncoder {
            encoder: self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None }),
        }
    }

    fn create_timestamp_query_set(&self, count: u32) -> Result<wgpu::QuerySet, GpuError> {
        if !self.capabilities.timestamp_queries() {
            return Err(GpuError::Capability {
                name: "timestamp queries",
            });
        }
        Ok(self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("molgfx frame timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count,
        }))
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn check_errors(&self) -> Result<(), GpuError> {
        self.errors.check()
    }

    fn resource_memory(&self) -> molgfx_gpu::ResourceMemory {
        self.resources.usage()
    }

    fn ray_query_limits(&self) -> Result<molgfx_gpu::RayQueryLimits, GpuError> {
        self.ray_query_limits_impl()
    }

    fn create_blas(&self, desc: &molgfx_gpu::BlasDesc<'_>) -> Result<Self::Blas, GpuError> {
        self.create_blas_impl(desc)
    }

    fn create_tlas(&self, desc: &molgfx_gpu::TlasDesc) -> Result<Self::Tlas, GpuError> {
        self.create_tlas_impl(desc)
    }

    fn set_tlas_instance(
        &self,
        tlas: &mut Self::Tlas,
        index: u32,
        instance: Option<molgfx_gpu::TlasInstance<'_, Self>>,
    ) -> Result<(), GpuError> {
        self.set_tlas_instance_impl(tlas, index, instance)
    }

    fn create_ray_query_bind_group_layout(
        &self,
        desc: &molgfx_gpu::RayQueryBindGroupLayoutDesc<'_>,
    ) -> Result<Self::BindGroupLayout, GpuError> {
        self.create_ray_query_bind_group_layout_impl(desc)
    }

    fn create_ray_query_bind_group(
        &self,
        desc: &molgfx_gpu::RayQueryBindGroupDesc<'_, Self>,
    ) -> Result<Self::BindGroup, GpuError> {
        self.create_ray_query_bind_group_impl(desc)
    }
}

#[cfg(test)]
#[path = "device_tests.rs"]
mod tests;
