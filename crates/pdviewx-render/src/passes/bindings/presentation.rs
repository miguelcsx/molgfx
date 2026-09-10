//! Active presentation inputs, shared between the native and browser frame paths.

use super::{FrameViews, PresentationSource};
use crate::passes::PassRegistry;
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, Device};

pub(super) struct PresentationBindings<D: Device> {
    pub(super) tonemap: PresentationSource<D::BindGroup>,
    pub(super) bloom_source: Option<PresentationSource<D::BindGroup>>,
    pub(super) bloom_horizontal_source: Option<D::BindGroup>,
    pub(super) bloom_vertical_source: Option<D::BindGroup>,
    pub(super) motion_blur: Option<PresentationSource<D::BindGroup>>,
}

pub(super) fn presentation_bindings<D: Device>(
    device: &D,
    passes: &PassRegistry<D>,
    views: &FrameViews<'_, D>,
) -> Option<PresentationBindings<D>> {
    let resolved = views.motion_blur.or(views.dof_output);
    let (bloom_source, bloom_horizontal_source, bloom_vertical_source) =
        if views.bloom_resolved.is_some() {
            let pass = passes.bloom.as_ref()?;
            let source = presentation_source(views, resolved, |source| {
                bloom_bind_group(device, pass, source)
            });
            let horizontal = bloom_bind_group(device, pass, views.bloom_horizontal_source?);
            let vertical = bloom_bind_group(device, pass, views.bloom_vertical_source?);
            (Some(source), Some(horizontal), Some(vertical))
        } else {
            (None, None, None)
        };
    let motion_blur = if views.motion_blur.is_some() {
        let pass = passes.motion_blur.as_ref()?;
        Some(presentation_source(views, views.dof_output, |source| {
            motion_blur_bind_group(device, pass, &passes.temporal.sampler, source, views.motion)
        }))
    } else {
        None
    };
    Some(PresentationBindings {
        tonemap: presentation_source(views, resolved, |source| {
            tonemap_bind_group(device, passes, source, views)
        }),
        bloom_source,
        bloom_horizontal_source,
        bloom_vertical_source,
        motion_blur,
    })
}

fn presentation_source<D: Device>(
    views: &FrameViews<'_, D>,
    effect: Option<&D::TextureView>,
    bind: impl Fn(&D::TextureView) -> D::BindGroup,
) -> PresentationSource<D::BindGroup> {
    match effect {
        Some(source) => PresentationSource::Effect(bind(source)),
        None => PresentationSource::History([bind(views.history_a), bind(views.history_b)]),
    }
}

fn tonemap_bind_group<D: Device>(
    device: &D,
    passes: &PassRegistry<D>,
    resolved: &D::TextureView,
    views: &FrameViews<'_, D>,
) -> D::BindGroup {
    device.create_bind_group(&BindGroupDesc {
        label: "tonemap temporal frame input",
        layout: &passes.tonemap.layout,
        entries: &[
            BindGroupEntry::Texture {
                binding: 0,
                view: resolved,
            },
            BindGroupEntry::Texture {
                binding: 1,
                view: views.depth,
            },
            BindGroupEntry::Texture {
                binding: 2,
                view: views.revealage,
            },
            BindGroupEntry::Texture {
                binding: 3,
                // The shader never samples this binding with bloom disabled.
                // Reuse a compatible view without retaining a dormant target.
                view: match views.bloom_resolved {
                    Some(view) => view,
                    None => views.albedo,
                },
            },
        ],
    })
}

fn bloom_bind_group<D: Device>(
    device: &D,
    pass: &crate::passes::BloomPass<D>,
    source: &D::TextureView,
) -> D::BindGroup {
    device.create_bind_group(&BindGroupDesc {
        label: "bloom stage source",
        layout: &pass.layout,
        entries: &[BindGroupEntry::Texture {
            binding: 0,
            view: source,
        }],
    })
}

fn motion_blur_bind_group<D: Device>(
    device: &D,
    pass: &crate::passes::MotionBlurPass<D>,
    sampler: &D::Sampler,
    source: &D::TextureView,
    motion: &D::TextureView,
) -> D::BindGroup {
    device.create_bind_group(&BindGroupDesc {
        label: "motion blur source and vectors",
        layout: &pass.layout,
        entries: &[
            BindGroupEntry::Texture {
                binding: 0,
                view: source,
            },
            BindGroupEntry::Texture {
                binding: 1,
                view: motion,
            },
            BindGroupEntry::Sampler {
                binding: 2,
                sampler,
            },
        ],
    })
}
