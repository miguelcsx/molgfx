//! Binding molecular sources into a scene, alone or placed as copies.

use super::Scene;
use crate::error::Error;
use crate::id::StructureId;
use crate::scene::hashing::structure_hash;
use crate::spec::{PatchOperation, ScenePatch, StructureSource};
use molgfx_math::Mat4;
use std::collections::BTreeMap;

impl Scene {
    /// Adds another provider-neutral source without copying coordinates.
    ///
    /// Announces the structure through an atomic `AddStructure` patch so the
    /// patch stream reaches remote scenes and browser viewers. The molecular
    /// data stays in the local binding map; the patch carries only the portable
    /// descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error if the source cannot be adapted or IDs are exhausted.
    pub fn add_source(
        &mut self,
        source: &molgfx_core::MolecularSource,
    ) -> Result<StructureId, Error> {
        Ok(self.add_structures(vec![Addition {
            source: source.clone(),
            of: None,
            placement: None,
        }])?[0])
    }

    /// Adds a copy of `of`'s molecular source at `placement`, sharing its data.
    ///
    /// The copy is a structure in its own right — its own identity, selections,
    /// representations and picks — that reads the same coordinates through a
    /// column-major 4×4 affine matrix. An assembly's copies are described this
    /// way.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown structure, a non-finite, non-affine or
    /// singular matrix, or an exhausted identity space.
    pub fn place(&mut self, of: StructureId, placement: [f32; 16]) -> Result<StructureId, Error> {
        Ok(self.place_many(of, &[placement])?[0])
    }

    /// Adds one copy of `of`'s source per placement, as a single revision.
    ///
    /// The copies share `of`'s coordinates and atom tables, so the cost is
    /// linear in the number of copies and independent of the atom count; the
    /// content hash is computed once per source and then cached.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown structure, any non-finite, non-affine or
    /// singular matrix, or an exhausted identity space; the scene is unchanged
    /// in every case.
    pub fn place_many(
        &mut self,
        of: StructureId,
        placements: &[[f32; 16]],
    ) -> Result<Vec<StructureId>, Error> {
        for placement in placements {
            StructureSource::validate_placement(placement)?;
        }
        let source = self.structures.get(&of).cloned().ok_or_else(|| {
            Error::InvalidSpec(format!("structure {} is not part of this scene", of.0))
        })?;
        self.add_structures(
            placements
                .iter()
                .map(|placement| Addition {
                    source: source.clone(),
                    of: Some(of),
                    placement: Some(*placement),
                })
                .collect(),
        )
    }

    /// Unbinds a structure and releases the molecular data only it held.
    ///
    /// Nothing in the specification may still draw or refer to the structure:
    /// remove its representations first. Identities are never reused. The
    /// revision advances, and earlier snapshots and patches that mention the
    /// structure can no longer be restored, so a caller that keeps undo history
    /// must drop it. Other structures keep their atom tables.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown structure or one still referred to; the
    /// scene is unchanged in both cases.
    pub fn remove_structure(&mut self, id: StructureId) -> Result<(), Error> {
        if !self.structures.contains_key(&id) {
            return Err(Error::InvalidSpec(format!(
                "structure {} is not part of this scene",
                id.0
            )));
        }
        let revision =
            self.spec.revision.checked_add(1).ok_or_else(|| {
                Error::InvalidSpec("scene revision space is exhausted".to_owned())
            })?;
        let mut spec = self.spec.clone();
        let _ = spec.structures.remove(&id);
        // Columns the structure owns go with it, in the specification and bound.
        spec.properties
            .retain(|_, property| property.structure != id);
        let property_bindings: BTreeMap<_, _> = self
            .property_bindings
            .iter()
            .filter(|(name, _)| spec.properties.contains_key(*name))
            .map(|(name, binding)| (name.clone(), binding.clone()))
            .collect();
        spec.revision = revision;
        let mut structures = self.structures.clone();
        let removed = structures.remove(&id);
        let reusable = self.structure_assets.without(id);
        // Resolving rejects a representation or item that still targets `id`.
        let resolution = super::runtime::resolve_reusing(
            &spec,
            &structures,
            &property_bindings,
            &self.overlay_bindings,
            &self.rows,
            Some(&reusable),
        )?;
        let prepared = self.prepare_resolution_playback(resolution, &structures)?;
        self.spec = spec;
        self.structures = structures;
        self.property_bindings = property_bindings;
        self.install_resolution(prepared);
        let _ = self.playback.remove(&id);
        if self.selected_scope == Some(id) {
            self.selected_scope = None;
        }
        if let Some(source) = removed {
            let identity = source.identity();
            if !self.structures.values().any(|s| s.identity() == identity) {
                self.rows.forget(identity);
                self.content_hashes.retain(|key, _| key.0 != identity);
            }
        }
        Ok(())
    }

    /// The content hash of a source, computed once per identity and
    /// coordinate revision.
    fn content_hash_of(&mut self, source: &molgfx_core::MolecularSource) -> Box<str> {
        let key = (source.identity(), source.coordinate_revision());
        self.content_hashes
            .entry(key)
            .or_insert_with(|| structure_hash(source))
            .clone()
    }

    fn add_structures(&mut self, additions: Vec<Addition>) -> Result<Vec<StructureId>, Error> {
        if additions.is_empty() {
            return Ok(Vec::new());
        }
        let exhausted = || Error::InvalidSpec("structure identity space is exhausted".to_owned());
        let count = u64::try_from(additions.len()).map_err(|_| exhausted())?;
        let first = self.next_structure;
        let next_structure = first.checked_add(count).ok_or_else(exhausted)?;
        let ids: Vec<StructureId> = (first..next_structure).map(StructureId).collect();
        let mut operations = Vec::with_capacity(additions.len());
        for (id, addition) in ids.iter().zip(&additions) {
            operations.push(PatchOperation::AddStructure {
                id: *id,
                source: StructureSource {
                    content_hash: self.content_hash_of(&addition.source),
                    uri: None,
                    format: None,
                    placement: addition.placement,
                },
            });
        }
        let patch = ScenePatch {
            base_revision: self.revision(),
            operations,
        };
        if self.extends_in_place() {
            self.extend_in_place(&ids, additions, &patch)?;
        } else {
            // The patch plan resolves against this binding map, so every
            // source must be present while the structural patch is prepared.
            for (id, addition) in ids.iter().zip(&additions) {
                let _ = self.structures.insert(*id, addition.source.clone());
            }
            if let Err(error) = self.apply(&patch) {
                for id in &ids {
                    let _ = self.structures.remove(id);
                }
                return Err(error);
            }
        }
        self.next_structure = next_structure;
        Ok(ids)
    }

    /// Whether adding a structure leaves every other part of the resolved
    /// scene as it is, so the new one can be appended without re-resolving.
    ///
    /// That holds when nothing in the specification reads across structures:
    /// no interaction channel, appearance rule, property, overlay or playback.
    /// Representations are fine, since each targets its own structure.
    fn extends_in_place(&self) -> bool {
        let spec = &self.spec;
        spec.focus.is_none()
            && spec.selected.is_none()
            && spec.hovered.is_none()
            && spec.muted.is_none()
            && spec.hidden.is_none()
            && spec.custom_interactions.is_empty()
            && spec.appearance.is_empty()
            && spec.properties.is_empty()
            && spec.volumes.is_empty()
            && spec.segmentations.is_empty()
            && spec.annotations.is_empty()
            && spec.measurements.is_empty()
            && spec.interactions.is_empty()
            && spec.trajectories.is_empty()
            && spec.ellipsoids.is_empty()
            && spec.planes.is_empty()
            && spec.assembly.is_none()
            && self.property_bindings.is_empty()
            && self.playback.is_empty()
            && self.selected_scope.is_none()
            && self.structure_assets.covers(
                self.structures.len(),
                self.structures.last_key_value().map(|(id, _)| *id),
            )
    }

    /// Appends the structures to the resolved scene and the specification.
    fn extend_in_place(
        &mut self,
        ids: &[StructureId],
        additions: Vec<Addition>,
        patch: &ScenePatch,
    ) -> Result<(), Error> {
        let revision =
            self.spec.revision.checked_add(1).ok_or_else(|| {
                Error::InvalidSpec("scene revision space is exhausted".to_owned())
            })?;
        // Every fallible step comes first, so a failure leaves the scene as it was.
        let mut assets = Vec::with_capacity(additions.len());
        let mut built = None;
        for addition in &additions {
            let known = addition.of.and_then(|of| self.structure_assets.asset(of));
            let asset = if let Some(asset) = known {
                asset.clone()
            } else {
                let handle = self.resolved.add_source(addition.source.clone())?;
                let asset = self
                    .resolved
                    .structure(handle)
                    .map(|placed| placed.asset().clone());
                built = Some(handle);
                asset.ok_or_else(|| {
                    Error::InvalidSpec("a bound structure has no placed asset".to_owned())
                })?
            };
            assets.push(asset);
        }
        for ((id, addition), (asset, operation)) in ids
            .iter()
            .zip(additions)
            .zip(assets.into_iter().zip(&patch.operations))
        {
            let PatchOperation::AddStructure {
                source: descriptor, ..
            } = operation
            else {
                continue;
            };
            // A source built above is already placed; a shared one is placed now.
            let handle = match built.take() {
                Some(handle) => handle,
                None => self.resolved.add_asset(&asset),
            };
            if let Some(matrix) = &addition.placement
                && let Some(placed) = self.resolved.structure_mut(handle)
            {
                placed.model_to_world = Mat4::from_cols_array(matrix);
            }
            self.structure_assets.push(*id, &addition.source, asset);
            let _ = self.spec.structures.insert(*id, descriptor.clone());
            let _ = self.structures.insert(*id, addition.source);
        }
        self.spec.revision = revision;
        Ok(())
    }
}

/// One structure to bind: its source, the structure it copies, and where.
struct Addition {
    source: molgfx_core::MolecularSource,
    of: Option<StructureId>,
    placement: Option<[f32; 16]>,
}
