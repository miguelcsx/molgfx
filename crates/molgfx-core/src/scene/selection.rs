//! Structure-scoped selection storage, algebra and spatial queries.
//!
//! Selection algebra is linear in selected rows. Spatial queries use one
//! persistent per-structure BVH and cost `O(f log n + candidates)` for `f`
//! focus atoms, without row-id aliasing between placed structures.

use crate::handle::{SelectionHandle, StructureHandle};
use crate::scene::{Scene, StoredSelection};
use crate::{AtomSelection, CoreError};

impl Scene {
    /// Stores one global row mask that applies independently to every placed
    /// structure.
    pub fn add_selection(&mut self, selection: AtomSelection) -> SelectionHandle {
        SelectionHandle(self.selections.insert(StoredSelection {
            global: Some(selection),
            scoped: Vec::new(),
        }))
    }

    /// Stores atom rows scoped to one placed structure.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed structure.
    pub fn add_structure_selection(
        &mut self,
        structure: StructureHandle,
        selection: AtomSelection,
    ) -> Result<SelectionHandle, CoreError> {
        if self.structures.get(structure.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        Ok(self.add_scoped_selection(vec![(structure, selection)]))
    }

    /// Selects water atoms using the structure's declared entity semantics.
    ///
    /// No residue-name heuristic is used: structures that do not declare a
    /// water entity produce an empty selection for that placement.
    #[must_use]
    pub fn select_water(&mut self) -> SelectionHandle {
        let scoped = self
            .structures
            .iter()
            .map(|(raw, placed)| {
                let mut rows = roaring::RoaringBitmap::new();
                let data = placed.structure.data();
                for chain in data.chains() {
                    let water = chain
                        .entity()
                        .and_then(|entity| data.topology.entities.kind(entity))
                        == Some(pdbiox::EntityKind::Water);
                    if water {
                        for residue in chain.residues() {
                            for atom in residue.atoms() {
                                rows.insert(atom.index().get());
                            }
                        }
                    }
                }
                (StructureHandle(raw), AtomSelection::Roaring(rows))
            })
            .collect();
        self.add_scoped_selection(scoped)
    }

    /// Resolves a global selection, or a mask scoped to exactly one structure.
    #[must_use]
    pub fn selection(&self, handle: SelectionHandle) -> Option<&AtomSelection> {
        let stored = self.selections.get(handle.0)?;
        match &stored.global {
            Some(selection) => Some(selection),
            None if stored.scoped.len() == 1 => stored.scoped.first().map(|(_, value)| value),
            None => None,
        }
    }

    /// Resolves the mask applicable to one placed structure.
    #[must_use]
    pub fn selection_for(
        &self,
        handle: SelectionHandle,
        structure: StructureHandle,
    ) -> Option<&AtomSelection> {
        let stored = self.selections.get(handle.0)?;
        match &stored.global {
            Some(selection) => Some(selection),
            None => stored
                .scoped
                .binary_search_by_key(&structure, |(handle, _)| *handle)
                .ok()
                .and_then(|index| stored.scoped.get(index))
                .map(|(_, selection)| selection),
        }
    }

    /// Boolean union over structure-scoped masks.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown selection.
    pub fn union_selections(
        &mut self,
        left: SelectionHandle,
        right: SelectionHandle,
    ) -> Result<SelectionHandle, CoreError> {
        self.combine_selections(left, right, AtomSelection::union)
    }

    /// Boolean intersection over structure-scoped masks.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown selection.
    pub fn intersect_selections(
        &mut self,
        left: SelectionHandle,
        right: SelectionHandle,
    ) -> Result<SelectionHandle, CoreError> {
        self.combine_selections(left, right, AtomSelection::intersect)
    }

    /// Boolean difference over structure-scoped masks.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown selection.
    pub fn difference_selections(
        &mut self,
        left: SelectionHandle,
        right: SelectionHandle,
    ) -> Result<SelectionHandle, CoreError> {
        self.combine_selections(left, right, AtomSelection::difference)
    }

    /// Keeps selected atoms belonging to molecular graph components of at
    /// least `minimum_atoms` rows.
    ///
    /// This operates on caller/`pdbiox`-supplied bonds. It is deterministic
    /// molecular-component filtering for surface sources, not a substitute
    /// for connected-component analysis of an already sampled scalar field.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an unknown selection or a zero threshold.
    pub fn select_molecular_components(
        &mut self,
        source: SelectionHandle,
        minimum_atoms: u32,
    ) -> Result<SelectionHandle, CoreError> {
        if minimum_atoms == 0 {
            return Err(CoreError::InvalidSelection {
                reason: "component size threshold must be at least one atom",
            });
        }
        if self.selections.get(source.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let mut scoped = Vec::with_capacity(self.structures.len());
        let mut parents = Vec::new();
        let mut sizes = Vec::new();
        for (raw, placed) in self.structures.iter() {
            let structure = StructureHandle(raw);
            let Some(selected) = self.selection_for(source, structure) else {
                continue;
            };
            let atom_count = placed.atoms.len();
            parents.clear();
            parents.resize(atom_count as usize, u32::MAX);
            sizes.clear();
            sizes.resize(atom_count as usize, 0u32);
            selected.for_each(atom_count, |atom| {
                if let (Some(parent), Some(size)) =
                    (parents.get_mut(atom as usize), sizes.get_mut(atom as usize))
                {
                    *parent = atom;
                    *size = 1;
                }
            });
            for bond in placed.structure.data().bonds.iter() {
                union_selected(
                    &mut parents,
                    &mut sizes,
                    bond.atom_a.get(),
                    bond.atom_b.get(),
                );
            }
            let mut retained = roaring::RoaringBitmap::new();
            selected.for_each(atom_count, |atom| {
                let root = find_root(&mut parents, atom);
                if sizes
                    .get(root as usize)
                    .is_some_and(|&size| size >= minimum_atoms)
                {
                    retained.insert(atom);
                }
            });
            scoped.push((structure, AtomSelection::Roaring(retained)));
        }
        Ok(self.add_scoped_selection(scoped))
    }

    /// Selects every atom not present in `selection`, per structure.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown selection.
    pub fn complement_selection(
        &mut self,
        selection: SelectionHandle,
    ) -> Result<SelectionHandle, CoreError> {
        if self.selections.get(selection.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let scoped = self
            .structures
            .iter()
            .filter_map(|(raw, placed)| {
                let structure = StructureHandle(raw);
                let selected = self.selection_for(selection, structure)?;
                Some((
                    structure,
                    AtomSelection::All.difference(selected, placed.atoms.len()),
                ))
            })
            .collect();
        Ok(self.add_scoped_selection(scoped))
    }

    fn combine_selections(
        &mut self,
        left: SelectionHandle,
        right: SelectionHandle,
        combine: fn(&AtomSelection, &AtomSelection, u32) -> AtomSelection,
    ) -> Result<SelectionHandle, CoreError> {
        if self.selections.get(left.0).is_none() || self.selections.get(right.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let scoped = self
            .structures
            .iter()
            .filter_map(|(raw, placed)| {
                let structure = StructureHandle(raw);
                let left = self.selection_for(left, structure)?;
                let right = self.selection_for(right, structure)?;
                Some((structure, combine(left, right, placed.atoms.len())))
            })
            .collect();
        Ok(self.add_scoped_selection(scoped))
    }

    pub(crate) fn add_scoped_selection(
        &mut self,
        mut scoped: Vec<(StructureHandle, AtomSelection)>,
    ) -> SelectionHandle {
        scoped.sort_unstable_by_key(|(handle, _)| *handle);
        SelectionHandle(self.selections.insert(StoredSelection {
            global: None,
            scoped,
        }))
    }

    /// Selects atoms within `distance` Å of a reference selection.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an unknown selection or invalid distance.
    pub fn select_within(
        &mut self,
        reference: SelectionHandle,
        distance: f32,
    ) -> Result<SelectionHandle, CoreError> {
        self.select_spatial(reference, distance, false)
    }

    /// Selects complete residues touching the distance neighbourhood.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an unknown selection or invalid distance.
    pub fn select_residues_within(
        &mut self,
        reference: SelectionHandle,
        distance: f32,
    ) -> Result<SelectionHandle, CoreError> {
        self.select_spatial(reference, distance, true)
    }

    fn select_spatial(
        &mut self,
        reference: SelectionHandle,
        distance: f32,
        complete_residues: bool,
    ) -> Result<SelectionHandle, CoreError> {
        if !distance.is_finite() || distance < 0.0 {
            return Err(CoreError::InvalidSelection {
                reason: "spatial distance must be finite and non-negative",
            });
        }
        if self.selections.get(reference.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let distance_sq = distance * distance;
        let mut scoped = Vec::with_capacity(self.structures.len());
        for (raw, placed) in self.structures.iter() {
            let structure = StructureHandle(raw);
            let Some(reference) = self.selection_for(reference, structure).cloned() else {
                continue;
            };
            self.spatial_result.clear();
            let coordinates = placed.atoms.coords().slice();
            let world_from_model = placed.model_to_world;
            let inverse_scale_bound = [
                molgfx_math::Vec3::X,
                molgfx_math::Vec3::Y,
                molgfx_math::Vec3::Z,
            ]
            .into_iter()
            .map(|axis| {
                world_from_model
                    .inverse()
                    .transform_vector3(axis)
                    .length_squared()
            })
            .sum::<f32>()
            .sqrt();
            let local_radius = distance * inverse_scale_bound;
            let hierarchy = placed.spatial_bvh()?;
            reference.for_each(placed.atoms.len(), |source| {
                let Some(source_position) = coordinates.get(source as usize).copied() else {
                    return;
                };
                let source_position = molgfx_math::Vec3::from_array(source_position);
                let source_world = world_from_model.transform_point3(source_position);
                hierarchy.sphere_candidates(
                    source_position,
                    local_radius,
                    &mut self.spatial_traversal,
                    &mut self.spatial_candidates,
                );
                for &candidate in &self.spatial_candidates {
                    let Some(candidate_position) = coordinates.get(candidate as usize).copied()
                    else {
                        continue;
                    };
                    let candidate_world = world_from_model
                        .transform_point3(molgfx_math::Vec3::from_array(candidate_position));
                    if source_world.distance_squared(candidate_world) <= distance_sq {
                        if complete_residues {
                            if let Some(residue) = placed.hierarchy.residue_of_atom(candidate) {
                                self.spatial_result
                                    .insert_range(placed.hierarchy.residue_atoms(residue));
                            }
                        } else {
                            self.spatial_result.insert(candidate);
                        }
                    }
                }
            });
            scoped.push((
                structure,
                AtomSelection::Roaring(self.spatial_result.clone()),
            ));
        }
        Ok(self.add_scoped_selection(scoped))
    }
}

fn find_root(parents: &mut [u32], atom: u32) -> u32 {
    let mut root = atom;
    while parents
        .get(root as usize)
        .is_some_and(|&parent| parent != root && parent != u32::MAX)
    {
        root = parents[root as usize];
    }
    let mut current = atom;
    while parents
        .get(current as usize)
        .is_some_and(|&parent| parent != root && parent != u32::MAX)
    {
        let next = parents[current as usize];
        parents[current as usize] = root;
        current = next;
    }
    root
}

fn union_selected(parents: &mut [u32], sizes: &mut [u32], left: u32, right: u32) {
    let (Some(&left_parent), Some(&right_parent)) =
        (parents.get(left as usize), parents.get(right as usize))
    else {
        return;
    };
    if left_parent == u32::MAX || right_parent == u32::MAX {
        return;
    }
    let mut left_root = find_root(parents, left);
    let mut right_root = find_root(parents, right);
    if left_root == right_root {
        return;
    }
    if sizes[left_root as usize] < sizes[right_root as usize] {
        std::mem::swap(&mut left_root, &mut right_root);
    }
    parents[right_root as usize] = left_root;
    sizes[left_root as usize] =
        sizes[left_root as usize].saturating_add(sizes[right_root as usize]);
}
