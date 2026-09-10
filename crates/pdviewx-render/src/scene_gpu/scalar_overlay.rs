//! Resolution of scalar-overlay scene handles to resident GPU resources.

use super::volume_slot::GpuVolumeResource;
use pdviewx_core::{Representation, ScalarVolume, Scene};
use pdviewx_gpu::Device;

pub(super) fn resolve<'gpu, 'scene, D: Device>(
    resources: &'gpu [GpuVolumeResource<D>],
    scene: &'scene Scene,
    representation: &Representation,
    fallback: &'gpu D::TextureView,
) -> (Option<&'scene ScalarVolume>, &'gpu D::TextureView, u64) {
    let Some(style) = representation.surface_scalar else {
        return (None, fallback, 0);
    };
    let Some(volume) = scene.volume(style.field) else {
        return (None, fallback, 0);
    };
    let Some((view, revision)) = resources
        .iter()
        .find(|resource| resource.handle == style.field)
        .and_then(GpuVolumeResource::binding)
    else {
        return (None, fallback, 0);
    };
    (Some(volume), view, revision)
}
