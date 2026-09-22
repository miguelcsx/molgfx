//! Per-placement top-level hardware acceleration for quality AO and shadows.
//!
//! The bottom-level structure is shared per record key; the top-level one is
//! not, because it carries the placement's model transform. Splitting them
//! means a moved placement re-instances one TLAS instead of rebuilding geometry
//! every sharer already holds — and that N representations over one selection
//! build one BLAS, not N.

use super::quality_hardware::{HardwareFailure, affine_rows, classify};
use molgfx_gpu::{
    AccelerationStructureBinding, AccelerationStructureFlags, AccelerationStructureUpdateMode,
    CommandEncoder, Device, GpuError, RayQueryBindGroupDesc, TlasDesc, TlasInstance,
};
use molgfx_math::Mat4;

#[derive(Debug)]
pub(super) struct PlacementAcceleration<D: Device> {
    tlas: Option<D::Tlas>,
    group: Option<D::BindGroup>,
    pending: bool,
    failure: Option<HardwareFailure>,
}

impl<D: Device> PlacementAcceleration<D> {
    pub(super) const fn new() -> Self {
        Self {
            tlas: None,
            group: None,
            pending: false,
            failure: None,
        }
    }

    pub(super) fn group(&self) -> Option<&D::BindGroup> {
        self.group.as_ref()
    }

    #[cfg(test)]
    pub(super) const fn failure(&self) -> Option<HardwareFailure> {
        self.failure
    }

    /// Instantiates the shared BLAS under this placement's transform.
    pub(super) fn sync(
        &mut self,
        device: &D,
        blas: Option<&D::Blas>,
        layout: Option<&D::BindGroupLayout>,
        transform: Mat4,
    ) {
        let (Some(blas), Some(layout)) = (blas, layout) else {
            self.disable(HardwareFailure::Unavailable);
            return;
        };
        if !device.capabilities().ray_query() {
            self.disable(HardwareFailure::Unavailable);
            return;
        }
        if let Err(error) = self.rebuild(device, layout, blas, transform) {
            self.disable(classify(&error));
        }
    }

    pub(super) fn record(&mut self, encoder: &mut D::CommandEncoder) {
        if !self.pending {
            return;
        }
        let Some(tlas) = self.tlas.as_ref() else {
            return;
        };
        match encoder.build_tlas(tlas) {
            Ok(()) => self.pending = false,
            Err(error) => self.disable(classify(&error)),
        }
    }

    fn rebuild(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        blas: &D::Blas,
        transform: Mat4,
    ) -> Result<(), GpuError> {
        let limits = device.ray_query_limits()?;
        if limits.max_tlas_instances == 0 || limits.max_bindings_per_shader_stage == 0 {
            return Err(GpuError::LimitExceeded {
                resource: "quality placement TLAS",
                limit: u64::from(limits.max_tlas_instances),
            });
        }
        if self.tlas.is_none() {
            let mut tlas = device.create_tlas(&TlasDesc {
                label: "quality placement TLAS",
                max_instances: 1,
                flags: AccelerationStructureFlags::ALLOW_UPDATE
                    | AccelerationStructureFlags::PREFER_FAST_TRACE,
                update_mode: AccelerationStructureUpdateMode::PreferUpdate,
            })?;
            device.set_tlas_instance(
                &mut tlas,
                0,
                Some(TlasInstance {
                    blas,
                    transform: affine_rows(transform),
                    custom_data: 0,
                    mask: u8::MAX,
                }),
            )?;
            self.group = Some(device.create_ray_query_bind_group(&RayQueryBindGroupDesc {
                label: "group3: quality ray-query scene",
                layout,
                entries: &[],
                acceleration_structures: &[AccelerationStructureBinding {
                    binding: 0,
                    tlas: &tlas,
                }],
            })?);
            self.tlas = Some(tlas);
        } else if let Some(tlas) = self.tlas.as_mut() {
            let instance = TlasInstance {
                blas,
                transform: affine_rows(transform),
                custom_data: 0,
                mask: u8::MAX,
            };
            device.set_tlas_instance(tlas, 0, Some(instance))?;
        }
        self.pending = true;
        self.failure = None;
        Ok(())
    }

    fn disable(&mut self, failure: HardwareFailure) {
        self.tlas = None;
        self.group = None;
        self.pending = false;
        self.failure = Some(failure);
    }
}
