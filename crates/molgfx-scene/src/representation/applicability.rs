//! Target compatibility measured with the same geometry extractors as rendering.

use super::CartoonStyle;
use super::form::{RepresentationFormSpec, RepresentationSpec};
use crate::Error;
use molgfx_core::{AtomSelection, MolecularSource};
use molgfx_geometry::{PolymerTraces, RibbonMesh};

/// Why a form cannot produce geometry for the requested molecular target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationRequirement {
    /// At least one selected atom with coordinates.
    Atoms,
    /// At least one bond with both endpoints in the selection.
    Bonds,
    /// A selected atom pair inside the instantaneous distance cutoff.
    DistancePairs,
    /// Three non-collinear neighbors within a selected central atom's cutoff.
    Triangles,
    /// Consecutive selected polymer guide atoms in a continuous trace.
    PolymerTrace,
    /// Connected selected carbohydrate rings.
    GlycosidicTrace,
    /// Complete selected molecular rings.
    Rings,
    /// Complete selected nucleotide pairs supported by upstream geometry.
    BasePairs,
    /// Native molecular topology required by the geometry extractor.
    MolecularTopology,
}

/// Result of checking a form against its actual selection.
///
/// An application can disable an unavailable menu entry and explain the
/// requirement. No fallback changes the user's requested form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RepresentationApplicability {
    /// Selected atom count, including isolated atoms.
    pub selected_atoms: u64,
    /// Missing geometry prerequisite; `None` means the form is drawable.
    pub missing: Option<RepresentationRequirement>,
}

impl RepresentationApplicability {
    /// Whether this exact form produces geometry on the target.
    #[must_use]
    pub const fn is_applicable(self) -> bool {
        self.missing.is_none()
    }
}

impl RepresentationSpec {
    pub(crate) fn applicability_at(
        &self,
        placed: &molgfx_core::PlacedStructure,
    ) -> Result<RepresentationApplicability, Error> {
        self.validate_values()?;
        let selection = placed
            .source
            .select_compiled(&*self.common.target.compiled()?)?;
        self.applicability_for_rows_at(&placed.source, &selection, Some(placed))
    }
    /// Checks target compatibility without changing the scene.
    ///
    /// # Errors
    /// Returns an error for invalid controls, an invalid query, or a geometry
    /// extractor failure. An unsuitable target is a successful diagnostic.
    pub fn applicability(
        &self,
        source: &MolecularSource,
    ) -> Result<RepresentationApplicability, Error> {
        self.validate_values()?;
        let selection = source.select_compiled(&*self.common.target.compiled()?)?;
        self.applicability_for_rows(source, &selection)
    }

    pub(crate) fn applicability_for_rows(
        &self,
        source: &MolecularSource,
        selection: &AtomSelection,
    ) -> Result<RepresentationApplicability, Error> {
        self.applicability_for_rows_at(source, selection, None)
    }

    fn applicability_for_rows_at(
        &self,
        source: &MolecularSource,
        selection: &AtomSelection,
        placed: Option<&molgfx_core::PlacedStructure>,
    ) -> Result<RepresentationApplicability, Error> {
        let count = u32::try_from(source.coordinates().len()).map_err(|_| {
            Error::InvalidSpec("molecular source exceeds addressable atom rows".to_owned())
        })?;
        let selected_atoms = selection.count(count);
        let missing = if selected_atoms == 0 {
            Some(RepresentationRequirement::Atoms)
        } else if let Some(placed) = placed
            && matches!(
                self.form,
                RepresentationFormSpec::DynamicBonds { .. }
                    | RepresentationFormSpec::Polyhedra { .. }
            )
        {
            self.distance_requirement_at(placed, selection)?
        } else {
            self.missing_requirement(source, selection)?
        };
        Ok(RepresentationApplicability {
            selected_atoms,
            missing,
        })
    }

    fn missing_requirement(
        &self,
        source: &MolecularSource,
        selection: &AtomSelection,
    ) -> Result<Option<RepresentationRequirement>, Error> {
        use RepresentationFormSpec as Form;
        use RepresentationRequirement as Need;
        if matches!(self.form, Form::Ribbons { .. } | Form::NewRibbons { .. })
            && source.molframe().is_some_and(|structure| {
                molgfx_geometry::has_selected_sugar_faces(structure, selection)
            })
        {
            return Ok(None);
        }
        if matches!(self.form, Form::Bonds { .. }) {
            return Ok((!source.topology().bonds.iter().any(|bond| {
                selection.contains(bond.atoms[0]) && selection.contains(bond.atoms[1])
            }))
            .then_some(Need::Bonds));
        }
        if matches!(
            self.form,
            Form::DynamicBonds { .. } | Form::Polyhedra { .. }
        ) {
            return self.distance_requirement(source, selection);
        }
        let requirement = match self.form {
            Form::Cartoon {
                style: CartoonStyle::Glycan,
                ..
            }
            | Form::Glycan { .. } => Need::GlycosidicTrace,
            Form::Cartoon { .. }
            | Form::NewRibbons { .. }
            | Form::Ribbons { .. }
            | Form::Backbone { .. }
            | Form::Trace { .. }
            | Form::Tube { .. }
            | Form::Putty { .. }
            | Form::NucleicAcid { .. } => Need::PolymerTrace,
            Form::PaperChain { .. } | Form::Bases { .. } => Need::Rings,
            Form::BasePairs { .. } => Need::BasePairs,
            _ => return Ok(None),
        };
        let Some(structure) = source.molframe() else {
            return Ok(Some(Need::MolecularTopology));
        };
        let drawable = self.has_topological_geometry(structure, selection, requirement)?;
        Ok((!drawable).then_some(requirement))
    }
    fn has_topological_geometry(
        &self,
        structure: &molframe::Structure,
        selection: &AtomSelection,
        requirement: RepresentationRequirement,
    ) -> Result<bool, Error> {
        use RepresentationFormSpec as Form;
        use RepresentationRequirement as Need;
        Ok(match requirement {
            Need::PolymerTrace => {
                let mut traces = PolymerTraces::default();
                let extract = if matches!(
                    self.form,
                    RepresentationFormSpec::Backbone { .. }
                        | RepresentationFormSpec::Trace { .. }
                        | RepresentationFormSpec::Tube { .. }
                        | RepresentationFormSpec::Putty { .. }
                ) {
                    molgfx_geometry::extract_phosphate_traces
                } else if matches!(
                    self.form,
                    Form::Cartoon {
                        style: CartoonStyle::Rocket,
                        ..
                    }
                ) {
                    molgfx_geometry::extract_protein_traces
                } else if matches!(
                    self.form,
                    Form::NewRibbons { .. }
                        | Form::Ribbons { .. }
                        | Form::Cartoon {
                            style: CartoonStyle::Ribbon | CartoonStyle::NucleicAcid,
                            ..
                        }
                        | Form::NucleicAcid { .. }
                ) {
                    molgfx_geometry::extract_contextual_polymer_traces
                } else {
                    molgfx_geometry::extract_polymer_traces
                };
                extract(
                    structure,
                    selection,
                    &[],
                    molgfx_geometry::CARTOON_GAP_CUTOFF,
                    &mut traces,
                )
                .map_err(|error| Error::InvalidSpec(error.to_string()))?;
                !traces.ranges().is_empty()
            }
            Need::GlycosidicTrace => {
                let mut traces = PolymerTraces::default();
                molgfx_geometry::extract_glycosidic_traces(structure, selection, &mut traces)
                    .map_err(|error| Error::InvalidSpec(error.to_string()))?;
                !traces.ranges().is_empty()
            }
            Need::BasePairs => {
                let selection = molgfx_geometry::paired_base_selection(structure, selection)
                    .map_err(|error| Error::InvalidSpec(error.to_string()))?;
                let mut mesh = RibbonMesh::default();
                molgfx_geometry::append_base_polygons(
                    structure,
                    &selection,
                    1.0,
                    0.25,
                    &mut mesh.vertices,
                    &mut mesh.indices,
                )
                .map_err(|error| Error::InvalidSpec(error.to_string()))?;
                !mesh.indices.is_empty()
            }
            Need::Rings => {
                let mut mesh = RibbonMesh::default();
                if let Form::PaperChain {
                    height,
                    max_ring_size,
                } = self.form
                {
                    mesh.generate_ring_bipyramids(
                        structure,
                        selection,
                        height,
                        max_ring_size,
                        u8::MAX,
                    )
                    .map_err(|error| Error::InvalidSpec(error.to_string()))?;
                } else {
                    molgfx_geometry::append_base_slabs(
                        structure,
                        selection,
                        0.3,
                        &mut mesh.vertices,
                        &mut mesh.indices,
                    )
                    .map_err(|error| Error::InvalidSpec(error.to_string()))?;
                }
                !mesh.indices.is_empty()
            }
            _ => false,
        })
    }
}
