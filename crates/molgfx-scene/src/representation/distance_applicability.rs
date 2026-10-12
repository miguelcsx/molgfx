//! Distance-defined availability over the same interval candidates as rendering.

use super::RepresentationRequirement;
use super::form::{RepresentationFormSpec, RepresentationSpec};
use crate::Error;
use molgfx_core::{AtomSelection, MolecularSource, PlacedStructure};
use molgfx_geometry::RibbonMesh;

impl RepresentationSpec {
    pub(super) fn distance_requirement(
        &self,
        source: &MolecularSource,
        selection: &AtomSelection,
    ) -> Result<Option<RepresentationRequirement>, Error> {
        let asset = molgfx_core::StructureAsset::from_source(
            molgfx_core::DatasetId::LEGACY,
            source.clone(),
        )
        .map_err(|error| Error::InvalidSpec(error.to_string()))?;
        self.distance_requirement_at(&PlacedStructure::from_asset(&asset), selection)
    }

    pub(super) fn distance_requirement_at(
        &self,
        placed: &PlacedStructure,
        selection: &AtomSelection,
    ) -> Result<Option<RepresentationRequirement>, Error> {
        use RepresentationFormSpec as Form;
        use RepresentationRequirement as Need;
        if let Form::DynamicBonds {
            cutoff,
            bond_radius,
        } = self.form
        {
            let mut compaction = Vec::new();
            molgfx_geometry::build_selection_compaction(
                selection,
                placed.atoms.len(),
                &mut compaction,
            )
            .map_err(|error| Error::InvalidSpec(error.to_string()))?;
            let mut scratch_scene = molgfx_core::Scene::new();
            let target = scratch_scene.add_selection(selection.clone());
            let mut representation = molgfx_core::Representation::new(
                molgfx_core::RepresentationTarget::Selection(target),
                molgfx_core::RepresentationKind::DynamicBonds,
            );
            representation.params.distance_cutoff = cutoff;
            representation.params.bond_radius = bond_radius;
            let mut bonds = Vec::new();
            molgfx_geometry::pack_bonds(placed, &representation, &compaction, &mut bonds)
                .map_err(|error| Error::InvalidSpec(error.to_string()))?;
            let rows: Vec<u32> = (0u32..)
                .zip(&compaction)
                .filter_map(|(row, &compact)| (compact != u32::MAX).then_some(row))
                .collect();
            let drawable = bonds.iter().any(|bond| {
                placed
                    .sampled_position(rows[bond.atom_a as usize])
                    .zip(placed.sampled_position(rows[bond.atom_b as usize]))
                    .is_some_and(|(a, b)| (b - a).length_squared() <= cutoff * cutoff)
            });
            return Ok((!drawable).then_some(Need::DistancePairs));
        }
        if let Form::Polyhedra { cutoff } = self.form {
            let mut mesh = RibbonMesh::default();
            mesh.generate_polyhedra(
                placed,
                selection,
                cutoff,
                molgfx_math::Rgba8::opaque(255, 255, 255),
            )
            .map_err(|error| Error::InvalidSpec(error.to_string()))?;
            let drawable = mesh.has_sampled_polyhedra(placed, cutoff);
            return Ok((!drawable).then_some(Need::Triangles));
        }
        Ok(None)
    }
}
