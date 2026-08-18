//! Reusable cartoon ribbon extrusion over transport-framed splines.

use super::profiles::{profile_scale, putty_radius, rocket_scale};
use super::traces::{
    PolymerTraces, PositionSource, extract_polymer_traces, extract_polymer_traces_from,
};
use pdviewx_core::{SecondaryStructure, TubeRadiusMapping};
use pdviewx_math::{CurveSample, Rgba8, TransportFrame, Vec3};

#[cfg(test)]
#[path = "ribbon_tests.rs"]
mod tests;

const PROFILE_SIDES: usize = 8;
const PROFILE: [[f32; 2]; PROFILE_SIDES] = [
    [1.0, 0.0],
    [0.707_106_77, 0.707_106_77],
    [0.0, 1.0],
    [-0.707_106_77, 0.707_106_77],
    [-1.0, 0.0],
    [-0.707_106_77, -0.707_106_77],
    [0.0, -1.0],
    [0.707_106_77, -0.707_106_77],
];

/// One gbuffer-ready cartoon vertex, aligned to two 16-byte lanes.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RibbonVertex {
    /// Model-space position.
    pub position: [f32; 3],
    /// Stable residue entity index.
    pub entity_id: u32,
    /// Model-space normal.
    pub normal: [f32; 3],
    /// Packed display colour.
    pub color: Rgba8,
}

/// Explicit quality and cross-section parameters.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RibbonParams {
    /// Maximum midpoint error in Ångström.
    pub tolerance: f32,
    /// Maximum samples per trace interval.
    pub max_steps: u8,
    /// Full width in Ångström.
    pub width: f32,
    /// Full thickness in Ångström.
    pub thickness: f32,
    /// Cross-section semantics applied along the shared spline.
    pub profile: SplineProfile,
    /// Reversible per-guide radius policy for tube profiles.
    pub radius_mapping: TubeRadiusMapping,
    /// Resolved display colour.
    pub color: Rgba8,
}

impl Default for RibbonParams {
    fn default() -> Self {
        Self {
            tolerance: 0.08,
            max_steps: 16,
            width: 1.2,
            thickness: 0.28,
            profile: SplineProfile::Cartoon,
            radius_mapping: TubeRadiusMapping::Constant,
            color: Rgba8::opaque(110, 165, 235),
        }
    }
}

/// Cross-section over the common adaptively sampled polymer spline.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum SplineProfile {
    /// Secondary-structure-aware ribbon with strand arrows and coil narrowing.
    #[default]
    Cartoon,
    /// Constant-radius circular tube.
    Tube,
    /// Discrete secondary-structure solids: helices as round columns, strands
    /// as flat arrows, coil as a thin cord. Where the cartoon profile reads as
    /// one continuous ribbon whose width tells the story, this one separates
    /// the elements by their cross-section, which is what makes a fold legible
    /// at assembly scale.
    Rocket,
}

/// Borrowed topology-aligned coordinates and interpolation fraction.
#[derive(Clone, Copy, Debug)]
pub struct InterpolatedCoordinates<'a> {
    /// Earlier decoded frame.
    pub start: &'a [[f32; 3]],
    /// Later decoded frame.
    pub end: &'a [[f32; 3]],
    /// Linear interpolation fraction in `[0, 1]`.
    pub alpha: f32,
}

/// Caller-owned reusable output and working storage.
#[derive(Clone, Debug, Default)]
pub struct RibbonMesh {
    /// Interleaved attributes consumed directly by the GPU.
    pub vertices: Vec<RibbonVertex>,
    /// Triangle indices, six per profile edge and trace interval.
    pub indices: Vec<u32>,
    samples: Vec<CurveSample>,
    frames: Vec<TransportFrame>,
    traces: PolymerTraces,
}

impl RibbonMesh {
    /// Regenerates a ribbon in `O(samples)` without allocating once capacity
    /// is sufficient. `entities[i]` anchors control point `i` to provenance.
    pub fn generate(&mut self, trace: &[Vec3], entities: &[u32], params: RibbonParams) {
        self.generate_styled(trace, entities, &[], params);
    }

    /// Regenerates a ribbon with one reversible secondary-structure style per
    /// control point. Missing styles resolve to coil without changing topology.
    pub fn generate_styled(
        &mut self,
        trace: &[Vec3],
        entities: &[u32],
        styles: &[SecondaryStructure],
        params: RibbonParams,
    ) {
        self.vertices.clear();
        self.indices.clear();
        let mut build = RibbonBuild {
            params,
            samples: &mut self.samples,
            frames: &mut self.frames,
            vertices: &mut self.vertices,
            indices: &mut self.indices,
        };
        append_ribbon(trace, entities, styles, &[], &mut build);
    }

    /// Generates every selected polymer trace in one structure into a single
    /// GPU-ready mesh, preserving chain breaks and guide-atom provenance.
    pub fn generate_structure(
        &mut self,
        structure: &pdbiox::Structure,
        selection: &pdviewx_core::AtomSelection,
        secondary: &[SecondaryStructure],
        max_gap: f32,
        params: RibbonParams,
    ) {
        extract_polymer_traces(structure, selection, secondary, max_gap, &mut self.traces);
        self.vertices.clear();
        self.indices.clear();
        let mut build = RibbonBuild {
            params,
            samples: &mut self.samples,
            frames: &mut self.frames,
            vertices: &mut self.vertices,
            indices: &mut self.indices,
        };
        for range in &self.traces.ranges {
            append_ribbon(
                &self.traces.points[range.points.clone()],
                &self.traces.entities[range.points.clone()],
                &self.traces.styles[range.points.clone()],
                &self.traces.properties[range.points.clone()],
                &mut build,
            );
        }
    }

    /// Generates a ribbon along a glycan's glycosidic tree.
    ///
    /// The traces come from connectivity rather than residue order, so each
    /// unbranched run draws as its own ribbon and a branch point starts a new
    /// one instead of stitching two arms into a false continuous chain.
    pub fn generate_glycan(
        &mut self,
        structure: &pdbiox::Structure,
        selection: &pdviewx_core::AtomSelection,
        params: RibbonParams,
    ) {
        super::glycan::extract_glycosidic_traces(structure, selection, &mut self.traces);
        self.vertices.clear();
        self.indices.clear();
        let mut build = RibbonBuild {
            params,
            samples: &mut self.samples,
            frames: &mut self.frames,
            vertices: &mut self.vertices,
            indices: &mut self.indices,
        };
        for range in &self.traces.ranges {
            append_ribbon(
                &self.traces.points[range.points.clone()],
                &self.traces.entities[range.points.clone()],
                &self.traces.styles[range.points.clone()],
                &self.traces.properties[range.points.clone()],
                &mut build,
            );
        }
    }

    /// Generates a structure ribbon at a topology-aligned trajectory sample.
    ///
    /// Guide positions are interpolated directly while extracting the trace;
    /// no full coordinate frame is materialized or copied on the host.
    pub fn generate_structure_interpolated(
        &mut self,
        structure: &pdbiox::Structure,
        selection: &pdviewx_core::AtomSelection,
        secondary: &[SecondaryStructure],
        max_gap: f32,
        coordinates: InterpolatedCoordinates<'_>,
        params: RibbonParams,
    ) {
        extract_polymer_traces_from(
            structure,
            selection,
            secondary,
            max_gap,
            PositionSource::Interpolated {
                start: coordinates.start,
                end: coordinates.end,
                alpha: coordinates.alpha,
            },
            &mut self.traces,
        );
        self.vertices.clear();
        self.indices.clear();
        let mut build = RibbonBuild {
            params,
            samples: &mut self.samples,
            frames: &mut self.frames,
            vertices: &mut self.vertices,
            indices: &mut self.indices,
        };
        for range in &self.traces.ranges {
            append_ribbon(
                &self.traces.points[range.points.clone()],
                &self.traces.entities[range.points.clone()],
                &self.traces.styles[range.points.clone()],
                &self.traces.properties[range.points.clone()],
                &mut build,
            );
        }
    }
}

struct RibbonBuild<'a> {
    params: RibbonParams,
    samples: &'a mut Vec<CurveSample>,
    frames: &'a mut Vec<TransportFrame>,
    vertices: &'a mut Vec<RibbonVertex>,
    indices: &'a mut Vec<u32>,
}

fn append_ribbon(
    trace: &[Vec3],
    entities: &[u32],
    styles: &[SecondaryStructure],
    properties: &[f32],
    build: &mut RibbonBuild<'_>,
) {
    pdviewx_math::sample_catmull_rom(
        trace,
        build.params.tolerance,
        build.params.max_steps,
        build.samples,
    );
    pdviewx_math::parallel_transport(build.samples, build.frames);
    if build.samples.len() < 2 || build.samples.len() != build.frames.len() {
        return;
    }
    let Ok(base_vertex) = u32::try_from(build.vertices.len()) else {
        return;
    };
    let half_width = build.params.width.abs() * 0.5;
    let half_thickness = build.params.thickness.abs() * 0.5;
    build.vertices.reserve(build.samples.len() * PROFILE_SIDES);
    for (sample, frame) in build.samples.iter().zip(build.frames.iter()) {
        let segment = usize::try_from(sample.segment).map_or(usize::MAX, |value| value);
        let entity_id = match entities.get(segment) {
            Some(&value) => value,
            None => u32::MAX,
        };
        let style = match styles.get(segment) {
            Some(&value) => value,
            None => SecondaryStructure::Coil,
        };
        let (width_scale, thickness_scale) = match build.params.profile {
            SplineProfile::Cartoon => profile_scale(style, sample.parameter, styles, segment),
            SplineProfile::Tube => {
                let radius = putty_radius(build.params, properties, segment, sample.parameter);
                let scale = radius / (build.params.width.abs() * 0.5).max(1.0e-6);
                (scale, scale)
            }
            SplineProfile::Rocket => rocket_scale(style, sample.parameter, styles, segment),
        };
        for [x, y] in PROFILE {
            let width = half_width * width_scale;
            let thickness = half_thickness * thickness_scale;
            let offset = frame.normal * (x * width) + frame.binormal * (y * thickness);
            let normal =
                (frame.normal * (x * thickness) + frame.binormal * (y * width)).normalize_or_zero();
            build.vertices.push(RibbonVertex {
                position: (sample.position + offset).to_array(),
                entity_id,
                normal: normal.to_array(),
                color: build.params.color,
            });
        }
    }
    build
        .indices
        .reserve((build.samples.len() - 1) * PROFILE_SIDES * 6);
    for ring in 0..build.samples.len() - 1 {
        append_ring(ring, base_vertex, build.indices);
    }
}

fn append_ring(ring: usize, base_vertex: u32, indices: &mut Vec<u32>) {
    let Some(base) = u32::try_from(ring * PROFILE_SIDES).ok() else {
        return;
    };
    let base = base.saturating_add(base_vertex);
    let stride = u32::try_from(PROFILE_SIDES).map_or(0, |value| value);
    for side in 0..PROFILE_SIDES {
        let current = u32::try_from(side).map_or(0, |value| value);
        let next = u32::try_from((side + 1) % PROFILE_SIDES).map_or(0, |value| value);
        indices.extend_from_slice(&[
            base + current,
            base + stride + current,
            base + stride + next,
            base + current,
            base + stride + next,
            base + next,
        ]);
    }
}
