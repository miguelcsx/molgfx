//! Size-dependent bind groups over persistent transient textures.

use crate::graph::TransientPool;
use crate::passes::{
    ALBEDO_RESOURCE, AO_DENOISED_RESOURCE, AO_RESOURCE, BLOOM_A_RESOURCE, BLOOM_B_RESOURCE,
    BLOOM_C_RESOURCE, COMPOSITE_RESOURCE, DEPTH_RESOURCE, DOF_RESOURCE, DOF_TILE_RESOURCE,
    HDR_RESOURCE, HISTORY_A_RESOURCE, HISTORY_B_RESOURCE, MOTION_BLUR_RESOURCE, MOTION_RESOURCE,
    NORMAL_RESOURCE, OIT_ACCUM_RESOURCE, OIT_REVEAL_RESOURCE, PassRegistry, SHADOW_RESOURCE,
};
use pdviewx_gpu::{BindGroupDesc, BindGroupEntry, Device};

#[derive(Debug)]
pub struct FrameBindings<D: Device> {
    pub(crate) ao: D::BindGroup,
    pub(crate) ao_denoise: D::BindGroup,
    pub(crate) lighting: D::BindGroup,
    pub(crate) oit: D::BindGroup,
    pub(crate) oit_composite: D::BindGroup,
    pub(crate) temporal: [[D::BindGroup; 2]; 2],
    pub(crate) dof_classify: D::BindGroup,
    pub(crate) dof: [D::BindGroup; 2],
    pub(crate) tonemap_history: [D::BindGroup; 2],
    pub(crate) tonemap_dof: D::BindGroup,
    pub(crate) tonemap_motion_blur: D::BindGroup,
    pub(crate) bloom_source_history: [D::BindGroup; 2],
    pub(crate) bloom_source_dof: D::BindGroup,
    pub(crate) bloom_source_motion_blur: D::BindGroup,
    pub(crate) bloom_horizontal_source: D::BindGroup,
    pub(crate) bloom_vertical_source: D::BindGroup,
    pub(crate) motion_blur_history: [D::BindGroup; 2],
    pub(crate) motion_blur_dof: D::BindGroup,
}

impl<D: Device> FrameBindings<D> {
    pub fn new(device: &D, pool: &TransientPool<D>, passes: &PassRegistry<D>) -> Option<Self> {
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
        let dof_classify = device.create_bind_group(&BindGroupDesc {
            label: "depth-of-field tile classification input",
            layout: &passes.depth_of_field.classify_layout,
            entries: &[BindGroupEntry::Texture {
                binding: 0,
                view: views.depth,
            }],
        });
        let dof = [
            dof_bind_group(
                device,
                passes,
                views.history_a,
                views.depth,
                views.dof_tiles,
            ),
            dof_bind_group(
                device,
                passes,
                views.history_b,
                views.depth,
                views.dof_tiles,
            ),
        ];
        let presentation = presentation_bindings(device, passes, &views);
        Some(Self {
            ao: base.ao,
            ao_denoise: base.ao_denoise,
            lighting: base.lighting,
            oit: base.oit,
            oit_composite: base.oit_composite,
            temporal,
            dof_classify,
            dof,
            tonemap_history: presentation.tonemap_history,
            tonemap_dof: presentation.tonemap_dof,
            tonemap_motion_blur: presentation.tonemap_motion_blur,
            bloom_source_history: presentation.bloom_source_history,
            bloom_source_dof: presentation.bloom_source_dof,
            bloom_source_motion_blur: presentation.bloom_source_motion_blur,
            bloom_horizontal_source: presentation.bloom_horizontal_source,
            bloom_vertical_source: presentation.bloom_vertical_source,
            motion_blur_history: presentation.motion_blur_history,
            motion_blur_dof: presentation.motion_blur_dof,
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
    dof_tiles: &'a D::TextureView,
    dof_output: &'a D::TextureView,
    motion: &'a D::TextureView,
    motion_blur: &'a D::TextureView,
    shadow: &'a D::TextureView,
    bloom_horizontal_source: &'a D::TextureView,
    bloom_vertical_source: &'a D::TextureView,
    bloom_resolved: &'a D::TextureView,
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
            dof_tiles: pool.view(DOF_TILE_RESOURCE)?,
            dof_output: pool.view(DOF_RESOURCE)?,
            motion: pool.view(MOTION_RESOURCE)?,
            motion_blur: pool.view(MOTION_BLUR_RESOURCE)?,
            shadow: pool.view(SHADOW_RESOURCE)?,
            bloom_horizontal_source: pool.view(BLOOM_A_RESOURCE)?,
            bloom_vertical_source: pool.view(BLOOM_B_RESOURCE)?,
            bloom_resolved: pool.view(BLOOM_C_RESOURCE)?,
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

struct PresentationBindings<D: Device> {
    tonemap_history: [D::BindGroup; 2],
    tonemap_dof: D::BindGroup,
    tonemap_motion_blur: D::BindGroup,
    bloom_source_history: [D::BindGroup; 2],
    bloom_source_dof: D::BindGroup,
    bloom_source_motion_blur: D::BindGroup,
    bloom_horizontal_source: D::BindGroup,
    bloom_vertical_source: D::BindGroup,
    motion_blur_history: [D::BindGroup; 2],
    motion_blur_dof: D::BindGroup,
}

fn presentation_bindings<D: Device>(
    device: &D,
    passes: &PassRegistry<D>,
    views: &FrameViews<'_, D>,
) -> PresentationBindings<D> {
    PresentationBindings {
        tonemap_history: [
            tonemap_bind_group(device, passes, views.history_a, views),
            tonemap_bind_group(device, passes, views.history_b, views),
        ],
        tonemap_dof: tonemap_bind_group(device, passes, views.dof_output, views),
        tonemap_motion_blur: tonemap_bind_group(device, passes, views.motion_blur, views),
        bloom_source_history: [
            bloom_bind_group(device, passes, views.history_a),
            bloom_bind_group(device, passes, views.history_b),
        ],
        bloom_source_dof: bloom_bind_group(device, passes, views.dof_output),
        bloom_source_motion_blur: bloom_bind_group(device, passes, views.motion_blur),
        bloom_horizontal_source: bloom_bind_group(device, passes, views.bloom_horizontal_source),
        bloom_vertical_source: bloom_bind_group(device, passes, views.bloom_vertical_source),
        motion_blur_history: [
            motion_blur_bind_group(device, passes, views.history_a, views.motion),
            motion_blur_bind_group(device, passes, views.history_b, views.motion),
        ],
        motion_blur_dof: motion_blur_bind_group(device, passes, views.dof_output, views.motion),
    }
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
    passes: &PassRegistry<D>,
    history: &D::TextureView,
    depth: &D::TextureView,
    tiles: &D::TextureView,
) -> D::BindGroup {
    device.create_bind_group(&BindGroupDesc {
        label: "depth-of-field gather inputs",
        layout: &passes.depth_of_field.resolve_layout,
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
                view: views.bloom_resolved,
            },
        ],
    })
}

fn bloom_bind_group<D: Device>(
    device: &D,
    passes: &PassRegistry<D>,
    source: &D::TextureView,
) -> D::BindGroup {
    device.create_bind_group(&BindGroupDesc {
        label: "bloom stage source",
        layout: &passes.bloom.layout,
        entries: &[BindGroupEntry::Texture {
            binding: 0,
            view: source,
        }],
    })
}

fn motion_blur_bind_group<D: Device>(
    device: &D,
    passes: &PassRegistry<D>,
    source: &D::TextureView,
    motion: &D::TextureView,
) -> D::BindGroup {
    device.create_bind_group(&BindGroupDesc {
        label: "motion blur source and vectors",
        layout: &passes.motion_blur.layout,
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
                sampler: &passes.temporal.sampler,
            },
        ],
    })
}
