//! Fit current and motion-history helix axes after coordinate interpolation.
use super::GpuScene;
use crate::passes::HelixFitPass;
use molgfx_gpu::{CommandEncoder, ComputePassDesc, Device};

impl<D: Device> GpuScene<D> {
    pub(crate) fn record_helix_fits(
        &self,
        encoder: &mut D::CommandEncoder,
        pipeline: &HelixFitPass<D>,
    ) {
        if !self.slots.iter().any(|slot| slot.ribbon.has_helix_fits()) {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "live helix fitting",
            timestamps: None,
        });
        for slot in &self.slots {
            slot.ribbon.record_helix_fits(&mut pass, &pipeline.pipeline);
        }
    }
}
