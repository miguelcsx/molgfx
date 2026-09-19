//! Size-dependent bind groups over persistent transient textures.

use crate::graph::TransientPool;
use crate::passes::{
    ALBEDO_RESOURCE, AO_DENOISED_RESOURCE, AO_RESOURCE, BLOOM_A_RESOURCE, BLOOM_B_RESOURCE,
    BLOOM_C_RESOURCE, COMPOSITE_RESOURCE, DEPTH_RESOURCE, DOF_RESOURCE, DOF_TILE_RESOURCE,
    HDR_RESOURCE, HISTORY_A_RESOURCE, HISTORY_B_RESOURCE, MOTION_BLUR_RESOURCE, MOTION_RESOURCE,
    NORMAL_RESOURCE, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE, PassRegistry, SHADOW_RESOURCE,
};
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, Device};

#[path = "bindings/presentation.rs"]
mod presentation;
use presentation::presentation_bindings;

#[derive(Debug)]
pub(crate) struct FrameBindings<D: Device> {
    pub(crate) ao: D::BindGroup,
    pub(crate) ao_denoise: D::BindGroup,
    pub(crate) lighting: D::BindGroup,
    pub(crate) oit: D::BindGroup,
    pub(crate) oit_composite: D::BindGroup,
    pub(crate) temporal: [[D::BindGroup; 2]; 2],
    pub(crate) dof_classify: Option<D::BindGroup>,
    pub(crate) dof: Option<[D::BindGroup; 2]>,
    pub(crate) tonemap: PresentationSource<D::BindGroup>,
    pub(crate) bloom_source: Option<PresentationSource<D::BindGroup>>,
    pub(crate) bloom_horizontal_source: Option<D::BindGroup>,
    pub(crate) bloom_vertical_source: Option<D::BindGroup>,
    pub(crate) motion_blur: Option<PresentationSource<D::BindGroup>>,
}

/// A stable effect output or one of the alternating temporal histories.
#[derive(Debug)]
pub(crate) enum PresentationSource<T> {
    Effect(T),
    History([T; 2]),
}

impl<T> PresentationSource<T> {
    pub(crate) fn get(&self, temporal_write: usize) -> Option<&T> {
        match self {
            Self::Effect(value) => Some(value),
            Self::History(values) => values.get(temporal_write),
        }
    }
}

impl<D: Device> FrameBindings<D> {
    pub(crate) fn new(
        device: &D,
        pool: &TransientPool<D>,
        passes: &PassRegistry<D>,
    ) -> Option<Self> {
        let views = FrameViews::new(pool)?;
        let base = base_bindings(device, passes, &views);
        let temporal = [
            [
                temporal_bind_group(
                    device,
                    passes,
                    views.hdr,
                    views.depth,
                    views.history_b,
                    views.motion,
                ),
                temporal_bind_group(
                    device,
                    passes,
                    views.hdr,
                    views.depth,
                    views.history_a,
                    views.motion,
                ),
            ],
            [
                temporal_bind_group(
                    device,
                    passes,
                    views.composite,
                    views.depth,
                    views.history_b,
                    views.motion,
                ),
                temporal_bind_group(
                    device,
                    passes,
                    views.composite,
                    views.depth,
                    views.history_a,
                    views.motion,
                ),
            ],
        ];
        let (dof_classify, dof) = if let Some(tiles) = views.dof_tiles {
            let pass = passes.depth_of_field.as_ref()?;
            let classify = device.create_bind_group(&BindGroupDesc {
                label: "depth-of-field tile classification input",
                layout: &pass.classify_layout,
                entries: &[BindGroupEntry::Texture {
                    binding: 0,
                    view: views.depth,
                }],
            });
            let resolve = [
                dof_bind_group(device, pass, views.history_a, views.depth, tiles),
                dof_bind_group(device, pass, views.history_b, views.depth, tiles),
            ];
            (Some(classify), Some(resolve))
        } else {
            (None, None)
        };
        let presentation = presentation_bindings(device, passes, &views)?;
        Some(Self {
            ao: base.ao,
            ao_denoise: base.ao_denoise,
            lighting: base.lighting,
            oit: base.oit,
            oit_composite: base.oit_composite,
            temporal,
            dof_classify,
            dof,
            tonemap: presentation.tonemap,
            bloom_source: presentation.bloom_source,
            bloom_horizontal_source: presentation.bloom_horizontal_source,
            bloom_vertical_source: presentation.bloom_vertical_source,
            motion_blur: presentation.motion_blur,
        })
    }
}

struct FrameViews<'a, D: Device> {
    depth: &'a D::TextureView,
    albedo: &'a D::TextureView,
    normal: &'a D::TextureView,
    ao: &'a D::TextureView,
    ao_denoised: &'a D::TextureView,
    hdr: &'a D::TextureView,
    history_a: &'a D::TextureView,
    history_b: &'a D::TextureView,
    accumulation: &'a D::TextureView,
    revealage: &'a D::TextureView,
    composite: &'a D::TextureView,
    dof_tiles: Option<&'a D::TextureView>,
    dof_output: Option<&'a D::TextureView>,
    motion: &'a D::TextureView,
    motion_blur: Option<&'a D::TextureView>,
    shadow: &'a D::TextureView,
    bloom_horizontal_source: Option<&'a D::TextureView>,
    bloom_vertical_source: Option<&'a D::TextureView>,
    bloom_resolved: Option<&'a D::TextureView>,
}

impl<'a, D: Device> FrameViews<'a, D> {
    fn new(pool: &'a TransientPool<D>) -> Option<Self> {
        Some(Self {
            depth: pool.view(DEPTH_RESOURCE)?,
            albedo: pool.view(ALBEDO_RESOURCE)?,
            normal: pool.view(NORMAL_RESOURCE)?,
            ao: pool.view(AO_RESOURCE)?,
            ao_denoised: pool.view(AO_DENOISED_RESOURCE)?,
            hdr: pool.view(HDR_RESOURCE)?,
            history_a: pool.view(HISTORY_A_RESOURCE)?,
            history_b: pool.view(HISTORY_B_RESOURCE)?,
            accumulation: pool.view(OIT_ACCUM_RESOURCE)?,
            revealage: pool.view(OIT_REVEAL_RESOURCE)?,
            composite: pool.view(COMPOSITE_RESOURCE)?,
            dof_tiles: pool.view(DOF_TILE_RESOURCE),
            dof_output: pool.view(DOF_RESOURCE),
            motion: pool.view(MOTION_RESOURCE)?,
            motion_blur: pool.view(MOTION_BLUR_RESOURCE),
            shadow: pool.view(SHADOW_RESOURCE)?,
            bloom_horizontal_source: pool.view(BLOOM_A_RESOURCE),
            bloom_vertical_source: pool.view(BLOOM_B_RESOURCE),
            bloom_resolved: pool.view(BLOOM_C_RESOURCE),
        })
    }
}

struct BaseBindings<D: Device> {
    ao: D::BindGroup,
    ao_denoise: D::BindGroup,
    lighting: D::BindGroup,
    oit: D::BindGroup,
    oit_composite: D::BindGroup,
}

fn base_bindings<D: Device>(
    device: &D,
    passes: &PassRegistry<D>,
    views: &FrameViews<'_, D>,
) -> BaseBindings<D> {
    let ao = device.create_bind_group(&BindGroupDesc {
        label: "ambient occlusion frame inputs",
        layout: &passes.ambient_occlusion.layout,
        entries: &[
            BindGroupEntry::Texture {
                binding: 0,
                view: views.depth,
            },
            BindGroupEntry::Texture {
                binding: 1,
                view: views.normal,
            },
        ],
    });
    let ao_denoise = device.create_bind_group(&BindGroupDesc {
        label: "occlusion denoise frame inputs",
        layout: &passes.ao_denoise.layout,
        entries: &[
            BindGroupEntry::Texture {
                binding: 0,
                view: views.ao,
            },
            BindGroupEntry::Texture {
                binding: 1,
                view: views.depth,
            },
            BindGroupEntry::Texture {
                binding: 2,
                view: views.normal,
            },
        ],
    });
    let lighting = device.create_bind_group(&BindGroupDesc {
        label: "deferred lighting frame inputs",
        layout: &passes.lighting.layout,
        entries: &[
            BindGroupEntry::Texture {
                binding: 0,
                view: views.albedo,
            },
            BindGroupEntry::Texture {
                binding: 1,
                view: views.normal,
            },
            BindGroupEntry::Texture {
                binding: 2,
                view: views.depth,
            },
            BindGroupEntry::Texture {
                binding: 3,
                view: views.ao_denoised,
            },
            BindGroupEntry::Texture {
                binding: 4,
                view: views.shadow,
            },
        ],
    });
    let oit = device.create_bind_group(&BindGroupDesc {
        label: "transparent opaque-scene inputs",
        layout: &passes.oit.layout,
        entries: &[
            BindGroupEntry::Texture {
                binding: 0,
                view: views.ao_denoised,
            },
            BindGroupEntry::Texture {
                binding: 1,
                view: views.depth,
            },
        ],
    });
    let oit_composite = device.create_bind_group(&BindGroupDesc {
        label: "weighted transparency composite inputs",
        layout: &passes.oit_composite.layout,
        entries: &[
            BindGroupEntry::Texture {
                binding: 0,
                view: views.hdr,
            },
            BindGroupEntry::Texture {
                binding: 1,
                view: views.accumulation,
            },
            BindGroupEntry::Texture {
                binding: 2,
                view: views.revealage,
            },
        ],
    });
    BaseBindings {
        ao,
        ao_denoise,
        lighting,
        oit,
        oit_composite,
    }
}

fn dof_bind_group<D: Device>(
    device: &D,
    pass: &crate::passes::DepthOfFieldPass<D>,
    history: &D::TextureView,
    depth: &D::TextureView,
    tiles: &D::TextureView,
) -> D::BindGroup {
    device.create_bind_group(&BindGroupDesc {
        label: "depth-of-field gather inputs",
        layout: &pass.resolve_layout,
        entries: &[
            BindGroupEntry::Texture {
                binding: 0,
                view: history,
            },
            BindGroupEntry::Texture {
                binding: 1,
                view: depth,
            },
            BindGroupEntry::Texture {
                binding: 2,
                view: tiles,
            },
        ],
    })
}

fn temporal_bind_group<D: Device>(
    device: &D,
    passes: &PassRegistry<D>,
    hdr: &D::TextureView,
    depth: &D::TextureView,
    history: &D::TextureView,
    motion: &D::TextureView,
) -> D::BindGroup {
    device.create_bind_group(&BindGroupDesc {
        label: "temporal frame inputs",
        layout: &passes.temporal.layout,
        entries: &[
            BindGroupEntry::Texture {
                binding: 0,
                view: hdr,
            },
            BindGroupEntry::Texture {
                binding: 1,
                view: depth,
            },
            BindGroupEntry::Texture {
                binding: 2,
                view: history,
            },
            BindGroupEntry::Sampler {
                binding: 3,
                sampler: &passes.temporal.sampler,
            },
            BindGroupEntry::Texture {
                binding: 4,
                view: motion,
            },
        ],
    })
}
