//! Dynamic relation resolution after trajectory interpolation.

use super::GpuScene;
use crate::passes::RelationResolvePass;
use pdviewx_gpu::{CommandEncoder, ComputePassDesc, ComputePassEncoder as _, Device};

impl<D: Device> GpuScene<D> {
    /// Resolves only dirty homogeneous anchor streams into the shared glyph table.
    pub fn record_dynamic_relations(
        &mut self,
        encoder: &mut D::CommandEncoder,
        resolver: &RelationResolvePass<D>,
        coordinates_changed: bool,
    ) -> bool {
        if !self.interactions.needs_resolve(coordinates_changed) {
            return false;
        }
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "dynamic relation resolution",
            timestamps: None,
        });
        for dispatch in self.interactions.resolve_dispatches() {
            let Some(pipeline) = resolver.pipeline(dispatch.pipeline) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.set_bind_group(1, dispatch.group, &[]);
            pass.dispatch(dispatch.groups[0], dispatch.groups[1], 1);
        }
        drop(pass);
        self.interactions.mark_resolved();
        true
    }
}
