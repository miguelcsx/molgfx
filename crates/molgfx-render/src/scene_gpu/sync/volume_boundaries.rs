//! Only active scalar levels retain derived geometry; styles do not key it.

use super::super::field_boundary::{BoundarySource, GpuFieldBoundary};
use super::super::volume_boundary::VolumeBoundary;
use super::GpuScene;
use crate::RenderError;
use molgfx_core::{Scene, VolumeRendering};
use molgfx_gpu::Device;

impl<D: Device> GpuScene<D> {
    pub(super) fn sync_volume_boundaries(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let revision = (scene.volume_revision(), scene.representation_revision());
        if self.volume_boundary_revision != Some(revision) {
            let mut keys: Vec<_> = scene
                .representations()
                .filter_map(|(_, representation)| {
                    if !representation.visible
                        || !matches!(
                            representation.volume.rendering,
                            VolumeRendering::IsoMesh | VolumeRendering::IsoDots
                        )
                    {
                        return None;
                    }
                    let handle = representation.volume_handle()?;
                    scene.volume(handle)?;
                    Some((handle, representation.params.isolevel.to_bits()))
                })
                .collect();
            keys.sort_unstable();
            keys.dedup();
            let mut previous = std::mem::take(&mut self.volume_boundaries)
                .into_iter()
                .peekable();
            self.volume_boundaries.reserve(keys.len());
            for key in keys {
                while previous.peek().is_some_and(|entry| entry.key < key) {
                    previous.next();
                }
                let entry = if previous.peek().is_some_and(|entry| entry.key == key) {
                    previous.next()
                } else {
                    None
                };
                self.volume_boundaries.push(match entry {
                    Some(entry) => entry,
                    None => VolumeBoundary {
                        key,
                        buffers: GpuFieldBoundary::default(),
                    },
                });
            }
            self.volume_boundary_revision = Some(revision);
        }
        let mut changed = false;
        for entry in &mut self.volume_boundaries {
            let (Some(grid), Some(revision)) = (
                scene.volume(entry.key.0),
                scene.volume_content_revision(entry.key.0),
            ) else {
                continue;
            };
            changed |= entry.buffers.sync(
                device,
                queue,
                &self.field_boundary_layout,
                BoundarySource::Scalar {
                    grid,
                    level: f32::from_bits(entry.key.1),
                },
                revision,
            )?;
        }
        Ok(changed)
    }
}
