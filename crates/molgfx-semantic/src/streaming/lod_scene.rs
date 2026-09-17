//! Scene-facing application of the semantic LOD decision.
//!
//! The core scene stays independent of this policy crate. This adapter lowers
//! visible coarse clusters to persistent particle records, reuses their
//! handles across frames, and cross-fades explicitly bound native detail
//! representations without rebuilding either side.

use super::plan::{LodFrame, LodIndex};
use crate::LodLevel;
use molgfx_core::{CoreError, PrimitiveHandle, RepresentationHandle, Scene, StructureHandle};

#[path = "lod_scene_helpers.rs"]
mod helpers;
use helpers::{
    bind_native, binding, clamp_unit, has_native_binding, set_binding_alpha, set_detail_alpha,
    unbind_native,
};

#[cfg(test)]
#[path = "lod_scene_tests.rs"]
mod tests;

/// Persistent coarse analytic particles owned by a semantic LOD controller.
///
/// Scene handles never cross scene identities. Cloning creates a fresh,
/// unbound controller because two controllers cannot own the same primitive.
#[derive(Debug, Default)]
pub struct LodScene {
    scene_identity: Option<u64>,
    bindings: Vec<LodBinding>,
    details: Vec<DetailBinding>,
    coarse: Vec<DetailBinding>,
    desired: Vec<super::plan::LodClusterKey>,
    missing: Vec<super::plan::LodClusterKey>,
    free: Vec<usize>,
}

impl Clone for LodScene {
    fn clone(&self) -> Self {
        Self::default()
    }
}

#[derive(Clone, Copy, Debug)]
struct LodBinding {
    key: super::plan::LodClusterKey,
    structure: StructureHandle,
    primitive: PrimitiveHandle,
    base_opacity: f32,
}

#[derive(Clone, Copy, Debug)]
struct DetailBinding {
    structure: StructureHandle,
    representation: RepresentationHandle,
    base_opacity: f32,
    base_visible: bool,
}

impl LodScene {
    /// Applies one selected frame while reusing the existing scene handles.
    ///
    /// Each visible residue, secondary-structure or domain cluster becomes one
    /// analytic sphere at the cluster centroid and radius. All records share
    /// the core scientific table and its single indirect draw, so the coarse
    /// path does not add one CPU draw call per biological cluster. Native
    /// representations registered with [`Self::bind_detail_representation`]
    /// are switched automatically from [`LodFrame::atom_structures`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed structure or
    /// [`CoreError::InvalidPrimitive`] for an inconsistent frame.
    pub fn apply(
        &mut self,
        scene: &mut Scene,
        index: &LodIndex,
        frame: &LodFrame,
    ) -> Result<(), CoreError> {
        self.bind_scene(scene);
        self.remove_stale(scene);
        self.desired.clear();
        self.desired
            .extend(frame.visible().iter().copied().filter(|key| {
                key.level != LodLevel::Atom && !has_native_binding(&self.coarse, key.structure)
            }));
        self.sync_bindings(scene, index)?;
        for binding in &self.bindings {
            let selected = frame.visible().binary_search(&binding.key).is_ok();
            set_binding_alpha(scene, *binding, f32::from(selected));
        }
        self.apply_details(scene, frame);
        self.apply_coarse(scene, frame);
        Ok(())
    }

    /// Cross-fades persistent coarse records between two selected frames.
    ///
    /// The transition is deterministic and changes only existing material and
    /// analytic-particle records. Bound native detail and coarse records remain
    /// resident, so the transition performs no topology rebuild or allocation.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed structure or
    /// [`CoreError::InvalidPrimitive`] for an inconsistent frame.
    pub fn apply_transition(
        &mut self,
        scene: &mut Scene,
        index: &LodIndex,
        from: &LodFrame,
        to: &LodFrame,
        weight: f32,
    ) -> Result<(), CoreError> {
        self.bind_scene(scene);
        self.remove_stale(scene);
        let weight = clamp_unit(weight);
        self.desired.clear();
        let mut from_row = 0usize;
        let mut to_row = 0usize;
        while from_row < from.visible().len() || to_row < to.visible().len() {
            let key = match (from.visible().get(from_row), to.visible().get(to_row)) {
                (Some(left), Some(right)) if left < right => {
                    from_row += 1;
                    *left
                }
                (Some(left), Some(right)) if right < left => {
                    to_row += 1;
                    *right
                }
                (Some(key), Some(_)) => {
                    from_row += 1;
                    to_row += 1;
                    *key
                }
                (Some(key), None) => {
                    from_row += 1;
                    *key
                }
                (None, Some(key)) => {
                    to_row += 1;
                    *key
                }
                (None, None) => break,
            };
            if key.level != LodLevel::Atom && !has_native_binding(&self.coarse, key.structure) {
                self.desired.push(key);
            }
        }
        self.sync_bindings(scene, index)?;
        for binding in &self.bindings {
            let was_visible = from.visible().binary_search(&binding.key).is_ok();
            let is_visible = to.visible().binary_search(&binding.key).is_ok();
            let alpha = match (was_visible, is_visible) {
                (true, true) => 1.0,
                (true, false) => 1.0 - weight,
                (false, true) => weight,
                (false, false) => 0.0,
            };
            set_binding_alpha(scene, *binding, alpha);
        }
        self.apply_detail_transition(scene, from, to, weight);
        self.apply_coarse_transition(scene, from, to, weight);
        Ok(())
    }

    /// Registers one existing native representation as the atom-detail view
    /// for a structure. Its current visibility and opacity become the values
    /// restored at full detail.
    ///
    /// Re-registering the same pair is a no-op. A representation may be bound
    /// to only one structure within this controller.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when either handle is absent.
    pub fn bind_detail_representation(
        &mut self,
        scene: &Scene,
        structure: StructureHandle,
        representation: RepresentationHandle,
    ) -> Result<(), CoreError> {
        self.bind_scene(scene);
        bind_native(&mut self.details, scene, structure, representation)
    }

    /// Releases one detail binding and restores its original presentation.
    pub fn unbind_detail_representation(
        &mut self,
        scene: &mut Scene,
        representation: RepresentationHandle,
    ) {
        unbind_native(&mut self.details, scene, representation);
    }

    /// Registers an existing volume, surface or other representation as the
    /// coarse view for a structure. It replaces generated coarse particles for
    /// that structure and cross-fades inversely to atom detail.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when either handle is absent.
    pub fn bind_coarse_representation(
        &mut self,
        scene: &Scene,
        structure: StructureHandle,
        representation: RepresentationHandle,
    ) -> Result<(), CoreError> {
        self.bind_scene(scene);
        bind_native(&mut self.coarse, scene, structure, representation)
    }

    /// Releases one coarse binding and restores its original presentation.
    pub fn unbind_coarse_representation(
        &mut self,
        scene: &mut Scene,
        representation: RepresentationHandle,
    ) {
        unbind_native(&mut self.coarse, scene, representation);
    }

    /// Number of persistent coarse analytic records currently owned.
    #[must_use]
    pub const fn primitive_count(&self) -> usize {
        self.bindings.len()
    }

    /// Number of native detail representations controlled by this adapter.
    #[must_use]
    pub const fn detail_representation_count(&self) -> usize {
        self.details.len()
    }

    /// Number of caller-owned coarse representations controlled by this adapter.
    #[must_use]
    pub const fn coarse_representation_count(&self) -> usize {
        self.coarse.len()
    }

    fn bind_scene(&mut self, scene: &Scene) {
        let identity = scene.cache_identity();
        if self.scene_identity == Some(identity) {
            return;
        }
        self.scene_identity = Some(identity);
        self.bindings.clear();
        self.details.clear();
        self.coarse.clear();
        self.desired.clear();
        self.missing.clear();
        self.free.clear();
    }

    fn sync_bindings(&mut self, scene: &mut Scene, index: &LodIndex) -> Result<(), CoreError> {
        self.missing.clear();
        self.free.clear();
        for &key in &self.desired {
            if self
                .bindings
                .binary_search_by_key(&key, |binding| binding.key)
                .is_err()
            {
                self.missing.push(key);
            }
        }
        for (row, binding) in self.bindings.iter().enumerate() {
            if self.desired.binary_search(&binding.key).is_err() {
                self.free.push(row);
            }
        }
        for key in self.missing.iter().copied() {
            if let Some(row) = self.free.pop() {
                let primitive = self.bindings[row].primitive;
                let binding = binding(scene, index, key, Some(primitive))?;
                self.bindings[row] = binding;
            } else {
                self.bindings.push(binding(scene, index, key, None)?);
            }
        }
        for row in self.free.drain(..).rev() {
            let stale = self.bindings.remove(row);
            scene.remove_primitive(stale.primitive);
        }
        self.bindings.sort_unstable_by_key(|binding| binding.key);
        Ok(())
    }

    fn remove_stale(&mut self, scene: &mut Scene) {
        self.bindings.retain(|binding| {
            let keep = scene.structure(binding.structure).is_some()
                && scene.primitive(binding.primitive).is_some();
            if !keep {
                scene.remove_primitive(binding.primitive);
            }
            keep
        });
        self.details.retain(|binding| {
            scene.structure(binding.structure).is_some()
                && scene.representation(binding.representation).is_some()
        });
        self.coarse.retain(|binding| {
            scene.structure(binding.structure).is_some()
                && scene.representation(binding.representation).is_some()
        });
    }

    fn apply_details(&self, scene: &mut Scene, frame: &LodFrame) {
        for &binding in &self.details {
            let selected = frame
                .atom_structures()
                .binary_search(&binding.structure)
                .is_ok();
            set_detail_alpha(scene, binding, f32::from(selected));
        }
    }

    fn apply_detail_transition(
        &self,
        scene: &mut Scene,
        from: &LodFrame,
        to: &LodFrame,
        weight: f32,
    ) {
        for &binding in &self.details {
            let was_visible = from
                .atom_structures()
                .binary_search(&binding.structure)
                .is_ok();
            let is_visible = to
                .atom_structures()
                .binary_search(&binding.structure)
                .is_ok();
            let alpha = match (was_visible, is_visible) {
                (true, true) => 1.0,
                (true, false) => 1.0 - weight,
                (false, true) => weight,
                (false, false) => 0.0,
            };
            set_detail_alpha(scene, binding, alpha);
        }
    }

    fn apply_coarse(&self, scene: &mut Scene, frame: &LodFrame) {
        for &binding in &self.coarse {
            let selected = frame
                .level(binding.structure)
                .is_some_and(|level| level != LodLevel::Atom);
            set_detail_alpha(scene, binding, f32::from(selected));
        }
    }

    fn apply_coarse_transition(
        &self,
        scene: &mut Scene,
        from: &LodFrame,
        to: &LodFrame,
        weight: f32,
    ) {
        for &binding in &self.coarse {
            let was_visible = from
                .level(binding.structure)
                .is_some_and(|level| level != LodLevel::Atom);
            let is_visible = to
                .level(binding.structure)
                .is_some_and(|level| level != LodLevel::Atom);
            let alpha = match (was_visible, is_visible) {
                (true, true) => 1.0,
                (true, false) => 1.0 - weight,
                (false, true) => weight,
                (false, false) => 0.0,
            };
            set_detail_alpha(scene, binding, alpha);
        }
    }
}
