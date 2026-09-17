//! Reusable cartoon ribbon extrusion over transport-framed splines.

use super::ends::append_end_caps;
use super::profiles::{BOX_FACE_SIDES, cross_section, profile_color, profile_scale, rocket_scale};
use super::traces::{PolymerTraces, extract_polymer_traces};
use molgfx_core::SecondaryStructure;
use molgfx_math::{CurveSample, Rgba8, TransportFrame, Vec3};

#[cfg(test)]
#[path = "ribbon_tests.rs"]
mod tests;

pub(super) const PROFILE_SIDES: usize = 8;

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

/// Parametric GPU deformation recipe for one ribbon vertex.
///
/// Four source atom rows define the Catmull–Rom interval. The reference frame
/// and profile coefficients let the vertex stage rotate the cross-section by
/// the minimal quaternion from the source tangent to the current tangent.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RibbonDeformation {
    /// Catmull–Rom source atom rows, or `u32::MAX` for static geometry.
    pub controls: [u32; 4],
    /// Local curve parameter, bit-cast left/right guide scalar indices, and a
    /// reserved lane. Keeping these in existing lanes preserves the 32-byte
    /// deformation row.
    pub parameter: [f32; 4],
}

impl RibbonDeformation {
    const STATIC: Self = Self {
        controls: [u32::MAX; 4],
        parameter: [0.0; 4],
    };
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
    /// Flat ribbon held in the plane of each residue rather than framed by
    /// parallel transport. A round tube would hide the one thing a glycan
    /// reader is looking for: a sugar ring is a plane, and the angle between
    /// consecutive rings is the glycosidic geometry, so the ribbon is given a
    /// face and that face is laid in the ring. The twist along the ribbon is
    /// then the molecule's own, and reading it off the picture is reading the
    /// structure.
    Twister,
}

/// Caller-owned reusable output and working storage.
#[derive(Clone, Debug, Default)]
pub struct RibbonMesh {
    /// Interleaved attributes consumed directly by the GPU.
    pub vertices: Vec<RibbonVertex>,
    /// Triangle indices, six per profile edge and trace interval.
    pub indices: Vec<u32>,
    /// One GPU deformation recipe per vertex.
    deformations: Vec<RibbonDeformation>,
    samples: Vec<CurveSample>,
    /// Per-interval twist demand, reused so a regenerate allocates nothing.
    demand: Vec<f32>,
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
        self.deformations.clear();
        self.traces.properties.clear();
        let mut build = RibbonBuild {
            params,
            samples: &mut self.samples,
            demand: &mut self.demand,
            frames: &mut self.frames,
            vertices: &mut self.vertices,
            indices: &mut self.indices,
            deformations: &mut self.deformations,
        };
        append_ribbon(trace, entities, styles, None, 0, &[], &mut build);
    }

    /// Generates every selected polymer trace in one structure into a single
    /// GPU-ready mesh, preserving chain breaks and guide-atom provenance.
    ///
    /// # Errors
    ///
    /// Returns [`crate::PackingError`] when a guide atom row cannot be encoded.
    pub fn generate_structure(
        &mut self,
        structure: &pdbiox::Structure,
        selection: &molgfx_core::AtomSelection,
        secondary: &[SecondaryStructure],
        max_gap: f32,
        params: RibbonParams,
    ) -> Result<(), crate::PackingError> {
        extract_polymer_traces(structure, selection, secondary, max_gap, &mut self.traces)?;
        self.vertices.clear();
        self.indices.clear();
        self.deformations.clear();
        let mut build = RibbonBuild {
            params,
            samples: &mut self.samples,
            demand: &mut self.demand,
            frames: &mut self.frames,
            vertices: &mut self.vertices,
            indices: &mut self.indices,
            deformations: &mut self.deformations,
        };
        for range in &self.traces.ranges {
            append_ribbon(
                &self.traces.points[range.points.clone()],
                &self.traces.entities[range.points.clone()],
                &self.traces.styles[range.points.clone()],
                u32::try_from(range.points.start).ok(),
                range.points.len(),
                plane_slice(&self.traces.normals, range.points.clone()),
                &mut build,
            );
        }
        Ok(())
    }

    /// Generates a ribbon along a glycan's glycosidic tree.
    ///
    /// The traces come from connectivity rather than residue order, so each
    /// unbranched run draws as its own ribbon and a branch point starts a new
    /// one instead of stitching two arms into a false continuous chain.
    pub fn generate_glycan(
        &mut self,
        structure: &pdbiox::Structure,
        selection: &molgfx_core::AtomSelection,
        params: RibbonParams,
    ) {
        super::glycan::extract_glycosidic_traces(structure, selection, &mut self.traces);
        self.vertices.clear();
        self.indices.clear();
        self.deformations.clear();
        let mut build = RibbonBuild {
            params,
            samples: &mut self.samples,
            demand: &mut self.demand,
            frames: &mut self.frames,
            vertices: &mut self.vertices,
            indices: &mut self.indices,
            deformations: &mut self.deformations,
        };
        for range in &self.traces.ranges {
            append_ribbon(
                &self.traces.points[range.points.clone()],
                &self.traces.entities[range.points.clone()],
                &self.traces.styles[range.points.clone()],
                None,
                0,
                plane_slice(&self.traces.normals, range.points.clone()),
                &mut build,
            );
        }
    }

    /// Empties every generated column, including the deformation recipes.
    ///
    /// A caller that appends its own geometry rather than calling one of the
    /// generators has to start from a clean mesh: the mesh is shared scratch,
    /// and a leftover recipe would otherwise survive `pad_static_deformations`
    /// and pull an unrelated vertex onto a spline it never belonged to.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.deformations.clear();
        self.traces.properties.clear();
    }

    /// Pads caller-appended static geometry so every vertex has one recipe.
    pub fn pad_static_deformations(&mut self) {
        self.deformations
            .resize(self.vertices.len(), RibbonDeformation::STATIC);
    }

    /// Packed GPU deformation recipes, one 32-byte row per vertex.
    #[must_use]
    pub fn deformation_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.deformations)
    }

    /// Compact B-factor stream aligned with extracted backbone guides.
    ///
    /// The stream contains no coordinates; deformation reads those directly
    /// from the structure asset's borrowed coordinate binding.
    #[must_use]
    pub fn radius_source_values(&self) -> &[f32] {
        &self.traces.properties
    }
}

pub(super) struct RibbonBuild<'a> {
    pub(super) params: RibbonParams,
    pub(super) samples: &'a mut Vec<CurveSample>,
    pub(super) demand: &'a mut Vec<f32>,
    pub(super) frames: &'a mut Vec<TransportFrame>,
    pub(super) vertices: &'a mut Vec<RibbonVertex>,
    pub(super) indices: &'a mut Vec<u32>,
    pub(super) deformations: &'a mut Vec<RibbonDeformation>,
}

/// Per-point ring planes for one trace, or nothing when the extractor produced
/// none.
///
/// Only a residue with a plane of its own contributes here, so a backbone
/// extractor leaves the column empty rather than filling it with a placeholder
/// per residue. Resolving that to an empty slice keeps the ribbon builder's
/// contract "planes are optional" instead of making every caller pad a column
/// it has no values for.
fn plane_slice(normals: &[Vec3], range: std::ops::Range<usize>) -> &[Vec3] {
    match normals.get(range) {
        Some(slice) => slice,
        None => &[],
    }
}

pub(super) fn control_rows(entities: &[u32], segment: usize) -> [u32; 4] {
    let Some(last) = entities.len().checked_sub(1) else {
        return [u32::MAX; 4];
    };
    let rows = [
        segment.saturating_sub(1),
        segment,
        (segment + 1).min(last),
        (segment + 2).min(last),
    ];
    let mut controls = [u32::MAX; 4];
    for (output, row) in controls.iter_mut().zip(rows) {
        let Some(entity) = entities.get(row) else {
            return [u32::MAX; 4];
        };
        let Some((molgfx_core::EntityKind::Atom, atom)) = molgfx_core::EntityId(*entity).unpack()
        else {
            return [u32::MAX; 4];
        };
        *output = atom;
    }
    controls
}

fn append_ribbon(
    trace: &[Vec3],
    entities: &[u32],
    styles: &[SecondaryStructure],
    property_base: Option<u32>,
    property_count: usize,
    normals: &[Vec3],
    build: &mut RibbonBuild<'_>,
) {
    // The adaptive sampler sees the shape of the curve and nothing else, so a
    // twisting ribbon has to tell it how far it turns; without that a glycan
    // earns one sample per sugar and folds. The demand is per interval, so a
    // run of coplanar rings still costs what a straight ribbon costs.
    if build.params.profile == SplineProfile::Twister {
        super::twist::twist_demand(trace, normals, build.demand);
    } else {
        build.demand.clear();
    }
    molgfx_math::sample_catmull_rom_demanding(
        trace,
        build.params.tolerance,
        build.params.max_steps,
        build.demand,
        build.samples,
    );
    molgfx_math::parallel_transport(build.samples, build.frames);
    if build.samples.len() < 2 || build.samples.len() != build.frames.len() {
        return;
    }
    if build.params.profile == SplineProfile::Twister {
        super::twist::orient_to_rings(build.samples, normals, build.frames);
    }
    let Ok(base_vertex) = u32::try_from(build.vertices.len()) else {
        return;
    };
    let flat = build.params.profile == SplineProfile::Twister;
    let half_width = build.params.width.abs() * 0.5;
    let half_thickness = build.params.thickness.abs() * 0.5;
    build.vertices.reserve(build.samples.len() * PROFILE_SIDES);
    build
        .deformations
        .reserve(build.samples.len() * PROFILE_SIDES);
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
            SplineProfile::Rocket => rocket_scale(style, sample.parameter, styles, segment),
            // The cross-section is constant: a sugar ring does not taper, and
            // varying the ribbon here would encode something the structure
            // does not say. Tubes likewise preserve their declared radius.
            SplineProfile::Tube | SplineProfile::Twister => (1.0, 1.0),
        };
        let controls = control_rows(entities, segment);
        let radius_controls = radius_control_rows(property_base, property_count, segment);
        let width = half_width * width_scale;
        let thickness = half_thickness * thickness_scale;
        for side in 0..PROFILE_SIDES {
            let (x, y, normal) = cross_section(flat, side, width, thickness);
            let offset = frame.normal * (x * width) + frame.binormal * (y * thickness);
            let normal =
                (frame.normal * normal[0] + frame.binormal * normal[1]).normalize_or_zero();
            build.vertices.push(RibbonVertex {
                position: (sample.position + offset).to_array(),
                entity_id,
                normal: normal.to_array(),
                color: profile_color(build.params.profile, y, build.params.color),
            });
            build.deformations.push(RibbonDeformation {
                controls,
                parameter: [
                    sample.parameter,
                    f32::from_bits(radius_controls[0]),
                    f32::from_bits(radius_controls[1]),
                    0.0,
                ],
            });
        }
    }
    build
        .indices
        .reserve((build.samples.len() - 1) * PROFILE_SIDES * 6);
    for ring in 0..build.samples.len() - 1 {
        append_ring(ring, base_vertex, flat, build.indices);
    }
    if flat {
        append_end_caps(entities, half_width, half_thickness, build);
    }
}

fn radius_control_rows(base: Option<u32>, count: usize, segment: usize) -> [u32; 2] {
    let Some(base) = base else {
        return [u32::MAX; 2];
    };
    let Some(right) = segment.checked_add(1) else {
        return [u32::MAX; 2];
    };
    if right >= count {
        return [u32::MAX; 2];
    }
    let (Ok(left), Ok(right)) = (u32::try_from(segment), u32::try_from(right)) else {
        return [u32::MAX; 2];
    };
    match (base.checked_add(left), base.checked_add(right)) {
        (Some(left), Some(right)) => [left, right],
        _ => [u32::MAX; 2],
    }
}

fn append_ring(ring: usize, base_vertex: u32, flat: bool, indices: &mut Vec<u32>) {
    let Some(base) = u32::try_from(ring * PROFILE_SIDES).ok() else {
        return;
    };
    let base = base.saturating_add(base_vertex);
    let stride = u32::try_from(PROFILE_SIDES).map_or(0, |value| value);
    for side in 0..PROFILE_SIDES {
        if flat && !BOX_FACE_SIDES[side % PROFILE_SIDES] {
            continue;
        }
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
