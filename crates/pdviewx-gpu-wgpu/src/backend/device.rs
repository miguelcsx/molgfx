//! The wgpu device: adapter selection, resource creation, capabilities.

use crate::convert;
use crate::surface::WgpuSurface;
use pdviewx_gpu::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BufferDesc, Capabilities,
    ComputePipelineDesc, DeviceDesc, GpuError, Opened, PowerPreference, RenderPipelineDesc,
    SamplerDesc, ShaderModuleDesc, TextureDesc, TextureViewDesc, WindowTarget,
};
use std::future::Future;

/// The wgpu implementation of the device abstraction.
#[derive(Debug)]
pub struct WgpuDevice {
    pub(crate) device: wgpu::Device,
    capabilities: Capabilities,
}

impl WgpuDevice {
    fn probe_capabilities(adapter: &wgpu::Adapter) -> Capabilities {
        let features = adapter.features();
        let limits = adapter.limits();
        let mut flags = pdviewx_gpu::CapabilityFlags::empty();
        let feature_map = [
            (
                wgpu::Features::EXPERIMENTAL_RAY_QUERY,
                pdviewx_gpu::CapabilityFlags::HARDWARE_RAY_TRACING,
            ),
            (
                wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
                pdviewx_gpu::CapabilityFlags::BINDLESS,
            ),
            (
                wgpu::Features::TIMESTAMP_QUERY,
                pdviewx_gpu::CapabilityFlags::TIMESTAMP_QUERIES,
            ),
            (
                wgpu::Features::SUBGROUP,
                pdviewx_gpu::CapabilityFlags::SUBGROUP_OPS,
            ),
        ];
        for (feature, capability) in feature_map {
            if features.contains(feature) {
                flags |= capability;
            }
        }
        Capabilities {
            flags,
            max_storage_buffer_bytes: limits.max_storage_buffer_binding_size,
            max_texture_dim: limits.max_texture_dimension_2d,
            max_texture_dim_3d: limits.max_texture_dimension_3d,
        }
    }

    /// Runs a closure under a pushed validation error scope, turning any
    /// captured error into a typed shader/pipeline failure.
    #[cfg(not(target_arch = "wasm32"))]
    fn validated<T>(&self, label: &'static str, create: impl FnOnce() -> T) -> Result<T, GpuError> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let value = create();
        let error: Option<wgpu::Error> = pollster::block_on(scope.pop());
        match error {
            None => Ok(value),
            Some(e) => Err(GpuError::ShaderCompile {
                label: label.to_owned(),
                detail: e.to_string(),
            }),
        }
    }

    /// Browser pipeline construction itself is synchronous, while error-scope
    /// resolution is asynchronous. Every composed WGSL unit is validated at
    /// build time; runtime device failures are reported by the browser device.
    #[cfg(target_arch = "wasm32")]
    fn validated<T>(
        &self,
        _label: &'static str,
        create: impl FnOnce() -> T,
    ) -> Result<T, GpuError> {
        Ok(create())
    }
}

impl pdviewx_gpu::Device for WgpuDevice {
    type Buffer = wgpu::Buffer;
    type Texture = wgpu::Texture;
    type TextureView = wgpu::TextureView;
    type Sampler = wgpu::Sampler;
    type ShaderModule = wgpu::ShaderModule;
    type BindGroupLayout = wgpu::BindGroupLayout;
    type BindGroup = wgpu::BindGroup;
    type Pipeline = crate::encoder::WgpuPipeline;
    type QuerySet = wgpu::QuerySet;
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
                    let surface = instance
                        .create_surface(target)
                        .map_err(|_| GpuError::NoAdapter)?;
                    Some(surface)
                }
                None => None,
            };
            #[cfg(target_arch = "wasm32")]
            let surface = match window {
                Some(canvas) => Some(
                    instance
                        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                        .map_err(|_| GpuError::NoAdapter)?,
                ),
                None => None,
            };

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: match power {
                        PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
                        PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
                    },
                    compatible_surface: surface.as_ref(),
                    force_fallback_adapter: false,
                    apply_limit_buckets: false,
                })
                .await
                .map_err(|_| GpuError::NoAdapter)?;

            let capabilities = Self::probe_capabilities(&adapter);
            let mut required_features = wgpu::Features::empty();
            if capabilities.timestamp_queries() {
                required_features |= wgpu::Features::TIMESTAMP_QUERY;
            }

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("pdviewx"),
                    required_features,
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                })
                .await
                .map_err(|_| GpuError::NoAdapter)?;

            let surface = surface.map(|surface| {
                let mut surface = WgpuSurface::new(surface, &adapter, &device);
                surface.attach_queue(queue.clone());
                surface
            });

            Ok(Opened {
                device: Self {
                    device,
                    capabilities,
                },
                queue: crate::queue::WgpuQueue { queue },
                surface,
            })
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_blocking(
        desc: &DeviceDesc,
        window: Option<WindowTarget>,
    ) -> Result<Opened<Self>, GpuError> {
        pollster::block_on(<Self as pdviewx_gpu::Device>::open_async(desc, window))
    }

    fn create_buffer(&self, desc: &BufferDesc) -> Result<wgpu::Buffer, GpuError> {
        Ok(self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(desc.label),
            size: desc.size,
            usage: convert::buffer_usage(desc.usage),
            mapped_at_creation: false,
        }))
    }

    fn create_texture(&self, desc: &TextureDesc) -> Result<wgpu::Texture, GpuError> {
        Ok(self.device.create_texture(&wgpu::TextureDescriptor {
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
        }))
    }

    fn create_texture_view(
        &self,
        texture: &wgpu::Texture,
        _desc: &TextureViewDesc,
    ) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor::default())
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
                    resource: buffer.as_entire_binding(),
                },
                BindGroupEntry::BufferRange {
                    binding,
                    buffer,
                    offset,
                    size,
                } => wgpu::BindGroupEntry {
                    binding: *binding,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
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
            label: Some("pdviewx frame timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count,
        }))
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
}
