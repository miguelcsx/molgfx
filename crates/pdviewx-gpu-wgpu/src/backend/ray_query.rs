//! wgpu 30 acceleration-structure resources and binding translation.

use super::device::WgpuDevice;
use pdviewx_gpu::{
    AccelerationIndexFormat, AccelerationStructureFlags, AccelerationStructureUpdateMode,
    BindGroupEntry, BlasDesc, BlasGeometrySizes, GpuError, RayQueryBindGroupDesc,
    RayQueryBindGroupLayoutDesc, RayQueryLimits, TlasDesc, TlasInstance,
};

pub(super) fn require_capability(capabilities: &pdviewx_gpu::Capabilities) -> Result<(), GpuError> {
    if capabilities.ray_query() {
        Ok(())
    } else {
        Err(GpuError::Capability { name: "ray query" })
    }
}

fn acceleration_flags(flags: AccelerationStructureFlags) -> wgpu::AccelerationStructureFlags {
    let mut converted = wgpu::AccelerationStructureFlags::empty();
    let mapping = [
        (
            AccelerationStructureFlags::ALLOW_UPDATE,
            wgpu::AccelerationStructureFlags::ALLOW_UPDATE,
        ),
        (
            AccelerationStructureFlags::PREFER_FAST_TRACE,
            wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        ),
        (
            AccelerationStructureFlags::PREFER_FAST_BUILD,
            wgpu::AccelerationStructureFlags::PREFER_FAST_BUILD,
        ),
        (
            AccelerationStructureFlags::LOW_MEMORY,
            wgpu::AccelerationStructureFlags::LOW_MEMORY,
        ),
    ];
    for (source, target) in mapping {
        if flags.contains(source) {
            converted |= target;
        }
    }
    converted
}

fn update_mode(mode: AccelerationStructureUpdateMode) -> wgpu::AccelerationStructureUpdateMode {
    match mode {
        AccelerationStructureUpdateMode::Build => wgpu::AccelerationStructureUpdateMode::Build,
        AccelerationStructureUpdateMode::PreferUpdate => {
            wgpu::AccelerationStructureUpdateMode::PreferUpdate
        }
    }
}

pub(super) fn geometry_flags(
    flags: pdviewx_gpu::AccelerationGeometryFlags,
) -> wgpu::AccelerationStructureGeometryFlags {
    if flags.contains(pdviewx_gpu::AccelerationGeometryFlags::OPAQUE) {
        wgpu::AccelerationStructureGeometryFlags::OPAQUE
    } else {
        wgpu::AccelerationStructureGeometryFlags::empty()
    }
}

pub(super) fn index_format(format: AccelerationIndexFormat) -> wgpu::IndexFormat {
    match format {
        AccelerationIndexFormat::Uint16 => wgpu::IndexFormat::Uint16,
        AccelerationIndexFormat::Uint32 => wgpu::IndexFormat::Uint32,
    }
}

impl WgpuDevice {
    pub(super) fn ray_query_limits_impl(&self) -> Result<RayQueryLimits, GpuError> {
        self.require_ray_query()?;
        let limits = self.device.limits();
        Ok(RayQueryLimits {
            max_blas_primitives: limits.max_blas_primitive_count,
            max_blas_geometries: limits.max_blas_geometry_count,
            max_tlas_instances: limits.max_tlas_instance_count,
            max_bindings_per_shader_stage: limits.max_acceleration_structures_per_shader_stage,
        })
    }

    pub(super) fn create_blas_impl(&self, desc: &BlasDesc<'_>) -> Result<wgpu::Blas, GpuError> {
        self.require_ray_query()?;
        let sizes = match desc.geometries {
            BlasGeometrySizes::Triangles(geometries) => {
                wgpu::BlasGeometrySizeDescriptors::Triangles {
                    descriptors: geometries
                        .iter()
                        .map(|geometry| wgpu::BlasTriangleGeometrySizeDescriptor {
                            vertex_format: wgpu::VertexFormat::Float32x3,
                            vertex_count: geometry.vertex_count,
                            index_format: geometry.indices.map(|indices| index_format(indices.0)),
                            index_count: geometry.indices.map(|indices| indices.1),
                            flags: geometry_flags(geometry.flags),
                        })
                        .collect(),
                }
            }
            BlasGeometrySizes::Aabbs(geometries) => wgpu::BlasGeometrySizeDescriptors::AABBs {
                descriptors: geometries
                    .iter()
                    .map(|geometry| wgpu::BlasAABBGeometrySizeDescriptor {
                        primitive_count: geometry.primitive_count,
                        flags: geometry_flags(geometry.flags),
                    })
                    .collect(),
            },
        };
        Ok(self.device.create_blas(
            &wgpu::CreateBlasDescriptor {
                label: Some(desc.label),
                flags: acceleration_flags(desc.flags),
                update_mode: update_mode(desc.update_mode),
            },
            sizes,
        ))
    }

    pub(super) fn create_tlas_impl(&self, desc: &TlasDesc) -> Result<wgpu::Tlas, GpuError> {
        self.require_ray_query()?;
        if desc.max_instances > self.device.limits().max_tlas_instance_count {
            return Err(GpuError::LimitExceeded {
                resource: desc.label,
                limit: u64::from(self.device.limits().max_tlas_instance_count),
            });
        }
        Ok(self.device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some(desc.label),
            max_instances: desc.max_instances,
            flags: acceleration_flags(desc.flags),
            update_mode: update_mode(desc.update_mode),
        }))
    }

    pub(super) fn set_tlas_instance_impl(
        &self,
        tlas: &mut wgpu::Tlas,
        index: u32,
        instance: Option<TlasInstance<'_, Self>>,
    ) -> Result<(), GpuError> {
        self.require_ray_query()?;
        let limit = u64::try_from(tlas.get().len()).map_err(|_| GpuError::LimitExceeded {
            resource: "TLAS instance capacity",
            limit: u64::MAX,
        })?;
        if instance
            .as_ref()
            .is_some_and(|value| value.custom_data > 0x00ff_ffff)
        {
            return Err(GpuError::LimitExceeded {
                resource: "TLAS custom data",
                limit: 0x00ff_ffff,
            });
        }
        let index = usize::try_from(index).map_err(|_| GpuError::LimitExceeded {
            resource: "TLAS instance index",
            limit,
        })?;
        let slot = tlas.get_mut_single(index).ok_or(GpuError::LimitExceeded {
            resource: "TLAS instance index",
            limit,
        })?;
        *slot = instance.map(|instance| {
            wgpu::TlasInstance::new(
                instance.blas,
                instance.transform,
                instance.custom_data,
                instance.mask,
            )
        });
        Ok(())
    }

    pub(super) fn create_ray_query_bind_group_impl(
        &self,
        desc: &RayQueryBindGroupDesc<'_, Self>,
    ) -> Result<wgpu::BindGroup, GpuError> {
        self.require_ray_query()?;
        let regular = desc.entries.iter().map(|entry| match entry {
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
        });
        let acceleration = desc
            .acceleration_structures
            .iter()
            .map(|entry| wgpu::BindGroupEntry {
                binding: entry.binding,
                resource: entry.tlas.as_binding(),
            });
        let entries: Vec<_> = regular.chain(acceleration).collect();
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(desc.label),
            layout: desc.layout,
            entries: &entries,
        }))
    }

    pub(super) fn create_ray_query_bind_group_layout_impl(
        &self,
        desc: &RayQueryBindGroupLayoutDesc<'_>,
    ) -> Result<wgpu::BindGroupLayout, GpuError> {
        self.require_ray_query()?;
        let regular = desc.entries.iter().map(|entry| wgpu::BindGroupLayoutEntry {
            binding: entry.binding,
            visibility: crate::convert::shader_stages(entry.visibility),
            ty: crate::convert::binding_type(entry.ty),
            count: None,
        });
        let acceleration =
            desc.acceleration_structures
                .iter()
                .map(|entry| wgpu::BindGroupLayoutEntry {
                    binding: entry.binding,
                    visibility: crate::convert::shader_stages(entry.visibility),
                    ty: wgpu::BindingType::AccelerationStructure {
                        vertex_return: false,
                    },
                    count: None,
                });
        let entries: Vec<_> = regular.chain(acceleration).collect();
        Ok(self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(desc.label),
                entries: &entries,
            }))
    }
}
