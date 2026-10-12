//! Scientific validity changes one retained uniform per affected batch, never its rows.

use super::{GpuInteractions, RelationVisualSource};
use molgfx_core::Scene;
use molgfx_gpu::{Device, Queue as _};

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct RelationCullConfig {
    pub(super) counts: [u32; 4],
}

impl<D: Device> GpuInteractions<D> {
    pub(super) fn sync_coordinate_validity(&mut self, queue: &D::Queue, scene: &Scene) {
        for (index, plan) in self.visual_plans.iter_mut().enumerate() {
            let inactive = match plan.source {
                RelationVisualSource::Scene(handle) => {
                    scene.relation_batch(handle).is_none_or(|batch| {
                        batch
                            .molecular_coordinates()
                            .is_some_and(|coordinates| !coordinates.matches(scene))
                    })
                }
                RelationVisualSource::Fallback | RelationVisualSource::Paged(_) => false,
            };
            plan.inactive = inactive;
            let Some(stream) = self
                .cull_streams
                .get_mut(index)
                .filter(|stream| stream.source == plan.source)
            else {
                continue;
            };
            let inactive = u32::from(inactive);
            if stream.counts[3] != inactive {
                stream.counts[3] = inactive;
                queue.write_buffer(
                    &stream.config,
                    0,
                    bytemuck::bytes_of(&RelationCullConfig {
                        counts: stream.counts,
                    }),
                );
            }
        }
    }
}
