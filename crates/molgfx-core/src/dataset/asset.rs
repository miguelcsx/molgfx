//! Immutable structure data shared by lightweight placements.
//!
//! Building an asset materializes renderer-facing atom columns once. Cloning
//! an asset or placement only increments one reference count; it never clones
//! coordinates, hierarchy offsets or derived atom columns.

use crate::{AtomTable, Column, DatasetError, DatasetId, Hierarchy, SecondaryStructure};
use molframe::ModelIndex;
use molgfx_math::{Aabb, Bvh, Mat4, SphereBounds};
use std::sync::{Arc, OnceLock};

#[derive(Debug)]
struct StructureAssetData {
    dataset: DatasetId,
    structure: molframe::Structure,
    atoms: Arc<AtomTable>,
    hierarchy: Arc<Hierarchy>,
    spatial_bvh: OnceLock<Result<Bvh, molgfx_math::BvhBuildError>>,
    spatial_bounds: Aabb,
    secondary_structure: Column<SecondaryStructure>,
}

/// Immutable renderer-facing structure columns shared across placements.
#[derive(Clone, Debug)]
pub struct StructureAsset {
    data: Arc<StructureAssetData>,
}

impl StructureAsset {
    /// Builds one shared asset from the first model.
    ///
    /// # Errors
    ///
    /// Returns a typed error when model zero has no dense coordinates or its
    /// atom count cannot fit the chunk-local `u32` address space.
    pub fn new(dataset: DatasetId, structure: &molframe::Structure) -> Result<Self, DatasetError> {
        Self::for_model(dataset, structure, ModelIndex::new(0))
    }

    /// Builds one shared asset from a specific model.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the model has no dense coordinates or its
    /// atom count cannot fit the chunk-local `u32` address space.
    pub fn for_model(
        dataset: DatasetId,
        structure: &molframe::Structure,
        model: ModelIndex,
    ) -> Result<Self, DatasetError> {
        let Some(coords) = crate::CoordRef::new(structure, model) else {
            return Err(DatasetError::MissingStructureModel);
        };
        u32::try_from(coords.len()).map_err(|_| DatasetError::StructureTooLarge)?;
        validate_topology_counts(structure)?;
        let Some(atoms) = AtomTable::from_structure(structure, model) else {
            return Err(DatasetError::MissingStructureModel);
        };
        let atoms = Arc::new(atoms);
        let hierarchy = Arc::new(Hierarchy::from_structure(structure));
        let spatial_bounds = atom_bounds(&atoms);
        let secondary_structure =
            Column::new(vec![SecondaryStructure::Unknown; hierarchy.residue_count()]);
        Ok(Self {
            data: Arc::new(StructureAssetData {
                dataset,
                structure: structure.clone(),
                atoms,
                hierarchy,
                spatial_bvh: OnceLock::new(),
                spatial_bounds,
                secondary_structure,
            }),
        })
    }

    /// Stable logical dataset identity.
    #[must_use]
    pub fn dataset_id(&self) -> DatasetId {
        self.data.dataset
    }

    /// Shared parser-owned structure retained for zero-copy coordinates.
    #[must_use]
    pub fn structure(&self) -> &molframe::Structure {
        &self.data.structure
    }

    /// Immutable atom columns materialized once for every placement.
    #[must_use]
    pub fn atoms(&self) -> &AtomTable {
        &self.data.atoms
    }

    /// Immutable topology offsets shared by every placement.
    #[must_use]
    pub fn hierarchy(&self) -> &Hierarchy {
        &self.data.hierarchy
    }

    /// Shared default secondary-structure column.
    #[must_use]
    pub fn secondary_structure(&self) -> &Column<SecondaryStructure> {
        &self.data.secondary_structure
    }

    /// Whether two handles reference exactly the same base allocation.
    #[must_use]
    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }

    /// Shared base hierarchy for selection, culling, surfaces and picking.
    ///
    /// # Errors
    ///
    /// Returns the cached typed build error when compact GPU offsets overflow.
    pub fn spatial_bvh(&self) -> Result<&Bvh, molgfx_math::BvhBuildError> {
        match self
            .data
            .spatial_bvh
            .get_or_init(|| Bvh::build(&sphere_bounds(&self.data.atoms)))
        {
            Ok(hierarchy) => Ok(hierarchy),
            Err(error) => Err(*error),
        }
    }

    pub(crate) fn shared_atoms(&self) -> Arc<AtomTable> {
        Arc::clone(&self.data.atoms)
    }

    pub(crate) fn shared_hierarchy(&self) -> Arc<Hierarchy> {
        Arc::clone(&self.data.hierarchy)
    }

    pub(crate) fn spatial_bounds(&self) -> Aabb {
        self.data.spatial_bounds
    }

    #[cfg(test)]
    pub(crate) fn spatial_bvh_is_ready(&self) -> bool {
        self.data.spatial_bvh.get().is_some()
    }

    /// Creates a lightweight placement sharing all base columns.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the transform contains a non-finite value.
    pub fn place(&self, model_to_world: Mat4) -> Result<StructureAssetPlacement, DatasetError> {
        StructureAssetPlacement::new(self.clone(), model_to_world)
    }
}

fn validate_topology_counts(structure: &molframe::Structure) -> Result<(), DatasetError> {
    let topology = &structure.data().topology;
    checked_topology_count(topology.models.len(), "model")?;
    checked_topology_count(topology.chains.len(), "chain")?;
    checked_topology_count(topology.residues.len(), "residue")
}

fn checked_topology_count(count: usize, table: &'static str) -> Result<(), DatasetError> {
    u32::try_from(count)
        .map(|_| ())
        .map_err(|_| DatasetError::StructureTopologyTooLarge { table })
}

/// One transform over a shared immutable [`StructureAsset`].
#[derive(Clone, Debug)]
pub struct StructureAssetPlacement {
    asset: StructureAsset,
    model_to_world: Mat4,
}

impl StructureAssetPlacement {
    /// Creates a placement without copying base structure data.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::InvalidPayload`] when the transform contains a
    /// non-finite component.
    pub fn new(asset: StructureAsset, model_to_world: Mat4) -> Result<Self, DatasetError> {
        if model_to_world
            .to_cols_array()
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(DatasetError::InvalidPayload {
                reason: "structure placement transform must be finite",
            });
        }
        Ok(Self {
            asset,
            model_to_world,
        })
    }

    /// Shared immutable structure data.
    #[must_use]
    pub const fn asset(&self) -> &StructureAsset {
        &self.asset
    }

    /// Model-to-world transform specific to this placement.
    #[must_use]
    pub const fn model_to_world(&self) -> Mat4 {
        self.model_to_world
    }

    /// World-space bound derived from shared coordinates and this transform.
    #[must_use]
    pub fn world_aabb(&self) -> Aabb {
        self.asset.spatial_bounds().transform(&self.model_to_world)
    }
}

/// The atom columns viewed as primitive bounds.
///
/// The hierarchy reads boxes straight from the position and radius columns the
/// table already owns. Materialising them first would cost twenty-four bytes
/// per atom for a temporary that is read once and dropped.
fn sphere_bounds(atoms: &AtomTable) -> SphereBounds<'_> {
    SphereBounds::new(atoms.coords().slice(), atoms.radius().values())
}

fn atom_bounds(atoms: &AtomTable) -> Aabb {
    molgfx_math::simd::spheres_aabb(atoms.coords().slice(), atoms.radius().values())
}

#[cfg(test)]
#[path = "asset_tests.rs"]
mod tests;
