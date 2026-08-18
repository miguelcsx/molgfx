//! The per-frame uniform block.
//!
//! Byte-identical to the shader's frame uniforms; uploading it is the only
//! per-frame write a camera-only change performs.

use pdviewx_core::{
    ClipSet, DensityVolume, MAX_CLIP_PLANES, Material, Representation, SurfaceKind,
};
use pdviewx_gpu::{Device, Queue};
use pdviewx_math::{Aabb, Camera, Mat4, Projection, Vec3};

pub(super) const SURFACE_GRID_MAX_DIMENSION: u32 = 192;
const SURFACE_GRID_TARGET_SPACING: f32 = 0.25;

/// Per-structure placement, shared by every representation of that structure.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelUniforms {
    /// World-from-model transform.
    pub model_to_world: Mat4,
    /// Model-from-world transform for local-space hierarchy traversal.
    pub world_to_model: Mat4,
    /// Previous world-from-model transform for object motion.
    pub previous_model_to_world: Mat4,
    /// Deterministic structure slot written to the picking gbuffer.
    pub structure_id: u32,
    padding: [u32; 3],
}

impl ModelUniforms {
    pub(super) fn new(
        model_to_world: Mat4,
        previous_model_to_world: Mat4,
        structure_id: u32,
    ) -> Self {
        Self {
            model_to_world,
            world_to_model: model_to_world.inverse(),
            previous_model_to_world,
            structure_id,
            padding: [0; 3],
        }
    }
}

/// The per-frame camera block, laid out exactly as the shader declares it.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FrameUniforms {
    /// View-from-world.
    pub view: Mat4,
    /// World-from-view, used by fullscreen analytic ray generation.
    pub inv_view: Mat4,
    /// Clip-from-view, reversed depth.
    pub proj: Mat4,
    /// Clip-from-world.
    pub view_proj: Mat4,
    /// View-from-clip, used by screen-space lighting passes.
    pub inv_proj: Mat4,
    /// Previous jittered clip-from-current jittered clip.
    pub reprojection: Mat4,
    /// Previous jittered clip-from-world, used for true geometry motion.
    pub previous_view_proj: Mat4,
    /// Width, height, 1/width, 1/height.
    pub viewport: [f32; 4],
    /// History, quality and publication flags followed by sample index.
    pub temporal: [f32; 4],
    /// Silhouette, cavity and depth-cue strengths followed by focus distance.
    pub illustration: [f32; 4],
    /// Focus distance, aperture scale, maximum blur radius and blade count.
    pub optics: [f32; 4],
    /// Shutter fraction and maximum motion-blur radius in pixels.
    pub motion_blur: [f32; 4],
    /// Projection kind in x: 0 perspective, 1 orthographic.
    pub projection_kind: [f32; 4],
    /// Light-view transform used by the scene-fit shadow pass.
    pub shadow_view: Mat4,
    /// World-from-light-view transform used by analytic shadow intersections.
    pub shadow_inv_view: Mat4,
    /// Reversed-depth orthographic light projection.
    pub shadow_projection: Mat4,
    /// Light clip-from-world transform used by lighting PCF.
    pub shadow_view_proj: Mat4,
    /// Background top, bottom, glow and display-grade controls.
    pub atmosphere: [[f32; 4]; 6],
    /// Environment, key and fill illumination controls.
    pub lighting: [[f32; 4]; 8],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TemporalFrame {
    pub(crate) jitter_pixels: [f32; 2],
    pub(crate) previous_view_proj: Option<Mat4>,
    pub(crate) shadow_view: Mat4,
    pub(crate) shadow_projection: Mat4,
    pub(crate) shadow_view_proj: Mat4,
    pub(crate) sample_index: u32,
    pub(crate) quality: bool,
    pub(crate) publication: bool,
    pub(crate) illustration: [f32; 4],
    pub(crate) optics: [f32; 4],
    pub(crate) motion_blur: [f32; 4],
    pub(crate) atmosphere: [[f32; 4]; 6],
    pub(crate) lighting: [[f32; 4]; 8],
}

impl FrameUniforms {
    /// Builds the block for one frame.
    #[must_use]
    pub fn new(camera: &Camera, width: u32, height: u32, temporal: &TemporalFrame) -> Self {
        let view = camera.view();
        let base_proj = camera.projection.matrix();
        // Viewport dimensions fit in 16 bits on every supported device, so
        // the float conversion is exact.
        let dim = |d: u32| match u16::try_from(d) {
            Ok(v) => f32::from(v),
            Err(_) => f32::from(u16::MAX),
        };
        let (w, h) = (dim(width), dim(height));
        let jitter_ndc = pdviewx_math::Vec3::new(
            2.0 * temporal.jitter_pixels[0] / w.max(1.0),
            -2.0 * temporal.jitter_pixels[1] / h.max(1.0),
            0.0,
        );
        let proj = Mat4::from_translation(jitter_ndc) * base_proj;
        let view_proj = proj * view;
        let history_valid = temporal.previous_view_proj.is_some();
        let previous = match temporal.previous_view_proj {
            Some(previous) => previous,
            None => view_proj,
        };
        Self {
            view,
            inv_view: view.inverse(),
            proj,
            view_proj,
            inv_proj: proj.inverse(),
            reprojection: previous * view_proj.inverse(),
            previous_view_proj: previous,
            shadow_view: temporal.shadow_view,
            shadow_inv_view: temporal.shadow_view.inverse(),
            shadow_projection: temporal.shadow_projection,
            shadow_view_proj: temporal.shadow_view_proj,
            viewport: [w, h, 1.0 / w.max(1.0), 1.0 / h.max(1.0)],
            temporal: [
                if history_valid { 1.0 } else { 0.0 },
                if temporal.quality { 1.0 } else { 0.0 },
                if temporal.publication { 1.0 } else { 0.0 },
                f32::from(
                    u16::try_from(temporal.sample_index.min(u32::from(u16::MAX)))
                        .map_or(u16::MAX, |value| value),
                ),
            ],
            illustration: temporal.illustration,
            optics: temporal.optics,
            motion_blur: temporal.motion_blur,
            projection_kind: [
                match camera.projection {
                    Projection::Perspective { .. } => 0.0,
                    Projection::Orthographic { .. } => 1.0,
                },
                0.0,
                0.0,
                0.0,
            ],
            atmosphere: temporal.atmosphere,
            lighting: temporal.lighting,
        }
    }
}

/// Per-representation parameters consumed by procedural shaders.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct RepresentationUniforms {
    /// Probe radius, level set, hit epsilon and minimum marching step.
    pub(super) surface: [f32; 4],
    /// Local-space grid minimum and padding.
    pub(super) grid_min: [f32; 4],
    /// Local-space cell size and padding.
    pub(super) grid_cell: [f32; 4],
    /// Surface kind, maximum steps, reentrant directions and presentation.
    pub(super) options: [u32; 4],
    /// Three grid dimensions followed by total cell count.
    pub(super) grid_size: [u32; 4],
    /// Point, surface-pattern and bond-wire dimensions in physical pixels.
    pub(super) visual: [f32; 4],
    /// World-space clipping plane equations.
    pub(super) clip_planes: [[f32; 4]; MAX_CLIP_PLANES],
    /// Active plane count followed by future cap flags.
    pub(super) clip_meta: [u32; 4],
    /// World-space position to caller scalar-grid coordinates.
    pub(super) overlay_world_to_voxel: [[f32; 4]; 4],
    /// Low, center and high scalar stops followed by contour interval.
    pub(super) overlay_domain: [f32; 4],
    /// Colors corresponding to the three scalar stops.
    pub(super) overlay_colors: [[f32; 4]; 3],
    /// Grid dimensions followed by an enabled sentinel.
    pub(super) overlay_size: [u32; 4],
    /// Contour half-width and normal sampling offset.
    pub(super) overlay_visual: [f32; 4],
    /// Perceptual roughness, dielectric specular strength and reserved lanes.
    pub(super) material: [f32; 4],
}

impl RepresentationUniforms {
    pub(super) fn new(
        representation: &Representation,
        bounds: Aabb,
        overlay_volume: Option<&DensityVolume>,
    ) -> Self {
        let gaussian = representation.params.surface_kind == SurfaceKind::Gaussian;
        let sigma = representation.params.gaussian_sigma.max(0.05);
        let probe = if representation.params.surface_kind == SurfaceKind::VanDerWaals {
            0.0
        } else if gaussian {
            sigma * 4.0
        } else {
            representation.params.probe_radius.max(0.0)
        };
        let minimum = bounds.min - Vec3::splat(probe);
        let maximum = bounds.max + Vec3::splat(probe);
        let extent = maximum - minimum;
        let max_divisions = dimension_f32(SURFACE_GRID_MAX_DIMENSION.saturating_sub(1));
        let cell = (extent.max_element() / max_divisions).max(SURFACE_GRID_TARGET_SPACING);
        let dimensions = [
            axis_cells(extent.x, cell),
            axis_cells(extent.y, cell),
            axis_cells(extent.z, cell),
        ];
        let total = dimensions.iter().copied().fold(1u32, u32::saturating_mul);
        let overlay = overlay_uniforms(representation, overlay_volume);
        Self {
            surface: [
                probe,
                representation.params.isolevel,
                if gaussian { sigma } else { 0.02 },
                0.02,
            ],
            grid_min: [minimum.x, minimum.y, minimum.z, 0.0],
            grid_cell: [cell, cell, cell, 0.0],
            options: [
                representation.params.surface_kind as u32,
                24,
                32,
                representation.params.surface_style as u32,
            ],
            grid_size: [dimensions[0], dimensions[1], dimensions[2], total],
            visual: [
                representation.params.point_size_pixels.max(1.0),
                representation.params.surface_pattern_spacing.max(0.05),
                representation.params.surface_pattern_width_pixels.max(0.25),
                if representation.kind == pdviewx_core::RepresentationKind::Lines {
                    representation.params.line_width_pixels.max(0.5)
                } else {
                    0.0
                },
            ],
            clip_planes: clip_planes(&representation.clipping),
            clip_meta: clip_meta(&representation.clipping),
            overlay_world_to_voxel: overlay.world_to_voxel,
            overlay_domain: overlay.domain,
            overlay_colors: overlay.colors,
            overlay_size: overlay.size,
            overlay_visual: overlay.visual,
            material: material_uniforms(representation.material),
        }
    }
}

pub(super) fn write_representation_uniforms<D: Device>(
    queue: &D::Queue,
    buffer: &D::Buffer,
    representation: &Representation,
    bounds: Aabb,
    overlay_volume: Option<&DensityVolume>,
) {
    let value = RepresentationUniforms::new(representation, bounds, overlay_volume);
    queue.write_buffer(buffer, 0, bytemuck::bytes_of(&value));
}

struct OverlayUniforms {
    world_to_voxel: [[f32; 4]; 4],
    domain: [f32; 4],
    colors: [[f32; 4]; 3],
    size: [u32; 4],
    visual: [f32; 4],
}

fn overlay_uniforms(
    representation: &Representation,
    volume: Option<&DensityVolume>,
) -> OverlayUniforms {
    let (Some(style), Some(volume)) = (representation.surface_scalar, volume) else {
        return OverlayUniforms {
            world_to_voxel: Mat4::IDENTITY.to_cols_array_2d(),
            domain: [0.0, 0.5, 1.0, 0.0],
            colors: [[0.0; 4]; 3],
            size: [1, 1, 1, 0],
            visual: [1.0, 0.0, 0.0, 0.0],
        };
    };
    let values = style.ramp.values();
    let colors = style.ramp.colors().map(color_f32);
    let (interval, width) = match style.contours {
        Some(contours) => (
            contours.interval.max(f32::EPSILON),
            contours.width_pixels.clamp(0.25, 8.0),
        ),
        None => (0.0, 1.0),
    };
    let dimensions = volume.dimensions();
    OverlayUniforms {
        world_to_voxel: volume.voxel_to_world().inverse().to_cols_array_2d(),
        domain: [values[0], values[1], values[2], interval],
        colors,
        size: [dimensions[0], dimensions[1], dimensions[2], 1],
        visual: [
            width,
            if style.sample_offset_angstrom.is_finite() {
                style.sample_offset_angstrom.clamp(-100.0, 100.0)
            } else {
                0.0
            },
            0.0,
            0.0,
        ],
    }
}

fn color_f32(color: pdviewx_math::Rgba8) -> [f32; 4] {
    let scale = 1.0 / 255.0;
    [
        f32::from(color.r) * scale,
        f32::from(color.g) * scale,
        f32::from(color.b) * scale,
        f32::from(color.a) * scale,
    ]
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ClipUniforms {
    pub(super) planes: [[f32; 4]; MAX_CLIP_PLANES],
    pub(super) meta: [u32; 4],
    pub(super) material: [f32; 4],
}

impl ClipUniforms {
    pub(super) fn for_material_and_clipping(material: Material, clipping: &ClipSet) -> Self {
        Self {
            planes: clip_planes(clipping),
            meta: clip_meta(clipping),
            material: material_uniforms(material),
        }
    }

    pub(super) fn for_mesh(mesh: &pdviewx_core::Mesh) -> Self {
        let mut value = Self::for_material_and_clipping(mesh.material(), &mesh.clipping());
        value.meta[2] = match mesh.face_visibility() {
            pdviewx_core::FaceVisibility::DoubleSided => 0,
            pdviewx_core::FaceVisibility::FrontOnly => 1,
            pdviewx_core::FaceVisibility::BackOnly => 2,
        };
        value
    }

    pub(super) fn new(representation: &Representation) -> Self {
        Self {
            planes: clip_planes(&representation.clipping),
            meta: clip_meta(&representation.clipping),
            material: material_uniforms(representation.material),
        }
    }
}

pub(super) fn material_uniforms(material: Material) -> [f32; 4] {
    let model = material.model_lanes();
    [
        material.perceptual_roughness(),
        material.specular_strength(),
        model[0],
        model[1],
    ]
}

pub(super) fn clip_planes(clipping: &ClipSet) -> [[f32; 4]; MAX_CLIP_PLANES] {
    let mut output = [[0.0; 4]; MAX_CLIP_PLANES];
    for (index, plane) in clipping.planes().iter().enumerate() {
        output[index] = [plane.normal.x, plane.normal.y, plane.normal.z, plane.offset];
    }
    output
}

pub(super) fn clip_meta(clipping: &ClipSet) -> [u32; 4] {
    [
        u32::try_from(clipping.planes().len()).map_or(0, |count| count),
        clipping.cap() as u32,
        0,
        0,
    ]
}

fn axis_cells(extent: f32, cell: f32) -> u32 {
    let mut cells = 2u32;
    while cells < SURFACE_GRID_MAX_DIMENSION
        && dimension_f32(cells.saturating_sub(1)) * cell < extent
    {
        cells += 1;
    }
    cells
}

fn dimension_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).map_or(u16::MAX, |converted| converted))
}

#[cfg(test)]
#[path = "uniforms_tests.rs"]
mod tests;
