//! Resource creation and optional ray-query behavior for the mock device.

use super::*;

impl molgfx_gpu::Device for MockDevice {
    type Buffer = MockBuffer;
    type Texture = MockTexture;
    type TextureView = MockView;
    type Sampler = ();
    type ShaderModule = ();
    type BindGroupLayout = MockBindGroupLayout;
    type BindGroup = u32;
    type Pipeline = u32;
    type QuerySet = MockQuerySet;
    type Blas = MockBlas;
    type Tlas = MockTlas;
    type CommandEncoder = MockEncoder;
    type Queue = MockQueue;
    type Surface = MockSurface;

    async fn open_async(
        _desc: &DeviceDesc,
        _window: Option<WindowTarget>,
    ) -> Result<Opened<Self>, GpuError> {
        std::future::ready(()).await;
        Ok(Self::opened())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_blocking(
        _desc: &DeviceDesc,
        _window: Option<WindowTarget>,
    ) -> Result<Opened<Self>, GpuError> {
        Ok(Self::opened())
    }

    fn create_buffer(&self, desc: &BufferDesc) -> Result<MockBuffer, GpuError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut buffers) = self.log.buffers.lock() {
            buffers.push((id, desc.label, desc.size));
        }
        Ok(MockBuffer {
            id,
            label: desc.label,
        })
    }

    fn create_texture(&self, desc: &TextureDesc) -> Result<MockTexture, GpuError> {
        if let Ok(mut textures) = self.log.textures.lock() {
            textures.push(desc.label);
        }
        if let Ok(mut extents) = self.log.texture_extents.lock() {
            extents.push((desc.label, [desc.width, desc.height, desc.depth]));
        }
        if let Ok(mut formats) = self.log.texture_formats.lock() {
            formats.push((desc.label, desc.format));
        }
        Ok(MockTexture(desc.label))
    }

    fn create_texture_view(&self, texture: &MockTexture, _desc: &TextureViewDesc) -> MockView {
        MockView(texture.0)
    }

    fn create_sampler(&self, _desc: &SamplerDesc) {}

    fn create_shader_module(&self, _desc: &ShaderModuleDesc<'_>) -> Result<(), GpuError> {
        Ok(())
    }

    fn create_bind_group_layout(&self, desc: &BindGroupLayoutDesc<'_>) -> MockBindGroupLayout {
        let mut counts = [0; 3];
        for entry in desc.entries {
            if !matches!(entry.ty, BindingType::Storage { .. }) {
                continue;
            }
            counts[0] += u32::from(entry.visibility.contains(ShaderStages::VERTEX));
            counts[1] += u32::from(entry.visibility.contains(ShaderStages::FRAGMENT));
            counts[2] += u32::from(entry.visibility.contains(ShaderStages::COMPUTE));
        }
        if let Ok(mut layouts) = self.log.bind_group_layouts.lock() {
            layouts.push((desc.label, counts));
        }
        MockBindGroupLayout {
            storage_counts: counts,
        }
    }

    fn create_bind_group(&self, desc: &BindGroupDesc<'_, Self>) -> u32 {
        if let Ok(mut bindings) = self.log.buffer_bindings.lock() {
            for entry in desc.entries {
                match entry {
                    molgfx_gpu::BindGroupEntry::BufferRange {
                        binding,
                        buffer,
                        offset,
                        size,
                    } => bindings.push((desc.label, *binding, buffer.id, *offset, *size)),
                    molgfx_gpu::BindGroupEntry::Buffer { binding, buffer } => {
                        bindings.push((desc.label, *binding, buffer.id, 0, 0));
                    }
                    molgfx_gpu::BindGroupEntry::Texture { .. }
                    | molgfx_gpu::BindGroupEntry::Sampler { .. } => {}
                }
            }
        }
        0
    }

    fn create_render_pipeline(&self, desc: &RenderPipelineDesc<'_, Self>) -> Result<u32, GpuError> {
        validate_pipeline_storage(
            desc.label,
            desc.layouts.iter().flatten().copied(),
            &[0, 1],
            self.capabilities.max_storage_buffers_per_shader_stage,
        )?;
        Ok(0)
    }

    fn create_compute_pipeline(
        &self,
        desc: &ComputePipelineDesc<'_, Self>,
    ) -> Result<u32, GpuError> {
        validate_pipeline_storage(
            desc.label,
            desc.layouts.iter().flatten().copied(),
            &[2],
            self.capabilities.max_storage_buffers_per_shader_stage,
        )?;
        Ok(0)
    }

    fn create_command_encoder(&self) -> MockEncoder {
        MockEncoder {
            log: Arc::clone(&self.log),
        }
    }

    fn create_timestamp_query_set(&self, _count: u32) -> Result<MockQuerySet, GpuError> {
        if self.capabilities.timestamp_queries() {
            Ok(MockQuerySet)
        } else {
            Err(GpuError::Capability {
                name: "timestamp queries",
            })
        }
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    fn ray_query_limits(&self) -> Result<molgfx_gpu::RayQueryLimits, GpuError> {
        if !self.capabilities.ray_query() {
            return Err(GpuError::Capability { name: "ray query" });
        }
        Ok(molgfx_gpu::RayQueryLimits {
            max_blas_primitives: u32::MAX,
            max_blas_geometries: 1,
            max_tlas_instances: 1,
            max_bindings_per_shader_stage: 1,
        })
    }

    fn create_blas(&self, _desc: &molgfx_gpu::BlasDesc<'_>) -> Result<MockBlas, GpuError> {
        self.ray_query_limits()?;
        fail_mock_ray_query(&self.log)?;
        self.log
            .acceleration_allocations
            .fetch_add(1, Ordering::Relaxed);
        Ok(MockBlas)
    }

    fn create_tlas(&self, desc: &molgfx_gpu::TlasDesc) -> Result<MockTlas, GpuError> {
        self.ray_query_limits()?;
        fail_mock_ray_query(&self.log)?;
        self.log
            .acceleration_allocations
            .fetch_add(1, Ordering::Relaxed);
        Ok(MockTlas {
            instances: vec![false; desc.max_instances as usize],
        })
    }

    fn set_tlas_instance(
        &self,
        tlas: &mut MockTlas,
        index: u32,
        instance: Option<molgfx_gpu::TlasInstance<'_, Self>>,
    ) -> Result<(), GpuError> {
        self.ray_query_limits()?;
        fail_mock_ray_query(&self.log)?;
        let limit = tlas.instances.len() as u64;
        let index = usize::try_from(index).map_err(|_| GpuError::LimitExceeded {
            resource: "mock TLAS instance",
            limit,
        })?;
        let Some(slot) = tlas.instances.get_mut(index) else {
            return Err(GpuError::LimitExceeded {
                resource: "mock TLAS instance",
                limit,
            });
        };
        *slot = instance.is_some();
        Ok(())
    }

    fn create_ray_query_bind_group_layout(
        &self,
        _desc: &molgfx_gpu::RayQueryBindGroupLayoutDesc<'_>,
    ) -> Result<MockBindGroupLayout, GpuError> {
        self.ray_query_limits().map(|_| MockBindGroupLayout {
            storage_counts: [0; 3],
        })
    }

    fn create_ray_query_bind_group(
        &self,
        _desc: &molgfx_gpu::RayQueryBindGroupDesc<'_, Self>,
    ) -> Result<u32, GpuError> {
        self.ray_query_limits()?;
        fail_mock_ray_query(&self.log)?;
        Ok(0)
    }
}

fn validate_pipeline_storage<'a>(
    label: &'static str,
    layouts: impl Iterator<Item = &'a MockBindGroupLayout>,
    stages: &[usize],
    limit: u32,
) -> Result<(), GpuError> {
    let mut counts = [0_u32; 3];
    for layout in layouts {
        for (sum, count) in counts.iter_mut().zip(layout.storage_counts) {
            *sum = sum.saturating_add(count);
        }
    }
    if stages.iter().any(|stage| counts[*stage] > limit) {
        return Err(GpuError::LimitExceeded {
            resource: label,
            limit: u64::from(limit),
        });
    }
    Ok(())
}
