//! Reusable cartoon ribbon extrusion over transport-framed splines.

use super::directions::{DirectionBuild, DirectionStorage};
use super::error::CartoonError;
use super::sweep::{RibbonTrace, append_ribbon};
use super::traces::{PolymerTraces, extract_polymer_traces};
use molgfx_core::{CartoonProfile, SecondaryStructure};
use molgfx_math::{CurveSample, Rgba8, TransportFrame, Vec3};

#[cfg(test)]
#[path = "ribbon_tests.rs"]
mod tests;

pub(super) const PROFILE_SIDES: usize = 16;

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

#[derive(Clone, Copy)]
enum GuideProperties {
    RadiusSources,
    Unmapped,
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
    /// anchor-mode lane. Keeping these in existing lanes preserves the 32-byte
    /// deformation row. Zero uses the spline; one anchors at the second control
    /// and uses the first-to-third vector as its direction.
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
    /// Secondary-structure cross-section width divided by thickness.
    pub aspect_ratio: f32,
    /// Strand arrow shoulder width relative to the body; zero disables arrows.
    pub arrow_factor: f32,
    /// Draws source-anchored polymer direction wedges.
    pub direction_wedges: bool,
    /// Cross-section of protein helices.
    pub helix_profile: CartoonProfile,
    /// Cross-section of nucleic-acid backbones.
    pub nucleic_profile: CartoonProfile,
    /// Cross-section semantics applied along the shared spline.
    pub profile: SplineProfile,
    /// Resolved display colour.
    pub color: Rgba8,
}

impl Default for RibbonParams {
    fn default() -> Self {
        Self {
            tolerance: 0.08,
            max_steps: 8,
            width: 1.2,
            thickness: 0.28,
            aspect_ratio: 5.0,
            arrow_factor: 1.5,
            direction_wedges: false,
            helix_profile: CartoonProfile::Elliptical,
            nucleic_profile: CartoonProfile::Square,
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
    directions: DirectionStorage,
}

impl RibbonMesh {
    /// Regenerates a ribbon in `O(samples)` without allocating once capacity
    /// is sufficient. `entities[i]` anchors control point `i` to provenance.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CartoonError`] for GPU index limits or a guide without a direction.
    pub fn generate(
        &mut self,
        trace: &[Vec3],
        entities: &[u32],
        params: RibbonParams,
    ) -> Result<(), CartoonError> {
        self.generate_styled(trace, entities, &[], params)
    }

    /// Regenerates a ribbon with one reversible secondary-structure style per
    /// control point. Missing styles resolve to coil without changing topology.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CartoonError`] for GPU index limits or a guide without a direction.
    pub fn generate_styled(
        &mut self,
        trace: &[Vec3],
        entities: &[u32],
        styles: &[SecondaryStructure],
        params: RibbonParams,
    ) -> Result<(), CartoonError> {
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
        let input = RibbonTrace {
            points: trace,
            entities,
            styles,
            property_base: None,
            property_count: 0,
            normals: &[],
            guides: &[],
        };
        append_ribbon(input, &mut build)?;
        if params.direction_wedges {
            let result = super::directions::append_trace(
                input,
                &self.samples,
                &self.frames,
                None,
                &mut DirectionBuild {
                    params,
                    vertices: &mut self.vertices,
                    indices: &mut self.indices,
                    deformations: &mut self.deformations,
                },
            );
            if let Err(error) = result {
                self.clear();
                return Err(error);
            }
        }
        Ok(())
    }

    /// Generates every selected polymer trace in one structure into a single
    /// GPU-ready mesh, preserving chain breaks and guide-atom provenance.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CartoonError`] when a guide atom row cannot be encoded
    /// or the generated geometry exceeds GPU index limits.
    pub fn generate_structure(
        &mut self,
        structure: &molframe::Structure,
        selection: &molgfx_core::AtomSelection,
        secondary: &[SecondaryStructure],
        max_gap: f32,
        params: RibbonParams,
    ) -> Result<(), CartoonError> {
        self.clear();
        extract_polymer_traces(structure, selection, secondary, max_gap, &mut self.traces)?;
        self.generate_traces(params, GuideProperties::RadiusSources)?;
        if params.direction_wedges {
            let result = self.directions.generate(
                structure,
                selection,
                secondary,
                max_gap,
                &mut DirectionBuild {
                    params,
                    vertices: &mut self.vertices,
                    indices: &mut self.indices,
                    deformations: &mut self.deformations,
                },
            );
            if let Err(error) = result {
                self.clear();
                return Err(error);
            }
        }
        Ok(())
    }

    /// Generates a ribbon along a glycan's glycosidic tree.
    ///
    /// The traces come from connectivity rather than residue order, so each
    /// unbranched run draws as its own ribbon and a branch point starts a new
    /// one instead of stitching two arms into a false continuous chain.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CartoonError`] for GPU index limits or a guide without a direction.
    pub fn generate_glycan(
        &mut self,
        structure: &molframe::Structure,
        selection: &molgfx_core::AtomSelection,
        params: RibbonParams,
    ) -> Result<(), CartoonError> {
        self.clear();
        if params.direction_wedges {
            return Err(CartoonError::DirectionProfile);
        }
        super::glycan::extract_glycosidic_traces(structure, selection, &mut self.traces);
        self.generate_traces(params, GuideProperties::Unmapped)
            .map_err(CartoonError::from)
    }

    fn generate_traces(
        &mut self,
        params: RibbonParams,
        properties: GuideProperties,
    ) -> Result<(), crate::PackingError> {
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
            let property_base = match properties {
                GuideProperties::RadiusSources => {
                    super::draw_limits::vertex_index(range.points.start).map(Some)
                }
                GuideProperties::Unmapped => Ok(None),
            };
            let result = property_base.and_then(|property_base| {
                append_ribbon(
                    RibbonTrace {
                        points: &self.traces.points[range.points.clone()],
                        entities: &self.traces.entities[range.points.clone()],
                        styles: &self.traces.styles[range.points.clone()],
                        property_base,
                        property_count: range.points.len(),
                        normals: plane_slice(&self.traces.normals, range.points.clone()),
                        guides: &self.traces.guides[range.points.clone()],
                    },
                    &mut build,
                )
            });
            if let Err(error) = result {
                build.vertices.clear();
                build.indices.clear();
                build.deformations.clear();
                return Err(error);
            }
        }
        Ok(())
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
