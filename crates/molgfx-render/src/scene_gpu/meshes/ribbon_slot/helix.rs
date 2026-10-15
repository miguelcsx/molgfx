//! Persistent fitting bindings share the ribbon's scalar storage allocation.
use super::{AssetArena, GpuStructure, RibbonSlot};
use crate::scene_gpu::buffers::buffer_entry;
use molgfx_gpu::{BindGroupDesc, ComputePassEncoder, Device};

impl<D: Device> RibbonSlot<D> {
    pub(in crate::scene_gpu) fn bind_helix_fits(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        structure: &GpuStructure<D>,
        arena: &AssetArena<D>,
    ) {
        self.fit_group = None;
        if self.fit_count == 0 {
            return;
        }
        let (Some(jobs), Some(fits)) = (self.fit_jobs.get(), self.radius_sources.get()) else {
            return;
        };
        self.fit_group = Some(device.create_bind_group(&BindGroupDesc {
            label: "live helix fitting",
            layout,
            entries: &[
                structure.coords_entry(arena, 0),
                structure.previous_coords_entry(arena, 1),
                structure.base_coords_entry(arena, 2),
                buffer_entry(3, jobs),
                buffer_entry(4, fits),
            ],
        }));
    }

    pub(in crate::scene_gpu) fn has_helix_fits(&self) -> bool {
        self.fit_group.is_some()
    }

    pub(in crate::scene_gpu) fn record_helix_fits(
        &self,
        pass: &mut impl ComputePassEncoder<D>,
        pipeline: &D::Pipeline,
    ) {
        if let Some(group) = &self.fit_group {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            let [width, height] =
                crate::scene_gpu::dispatch::workgroups_2d(u64::from(self.fit_count));
            pass.dispatch(width, height, 1);
        }
    }
}
