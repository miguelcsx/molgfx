//! Stable compact storage for reusable ligand topology pose batches.

use crate::{
    CoreError, EntityKind, EntityRef, LicoriceTemplate, LigandPose, LigandPoseBatch,
    LigandPoseBatchHandle, Scene, StructureHandle,
};

impl Scene {
    /// Moves one topology and its rigid candidates without expanding geometry.
    ///
    /// Scene and GPU residency are both `O(template + poses)`. The vertex
    /// stage expands topology/pose pairs analytically at draw time, without a
    /// Cartesian instance table or per-frame upload.
    ///
    /// # Errors
    ///
    /// Returns without insertion for a stale owner, overflowing transform or
    /// an instance count that cannot be represented by one GPU draw.
    pub fn add_licorice_poses(
        &mut self,
        owner: StructureHandle,
        template: LicoriceTemplate,
        poses: Vec<LigandPose>,
    ) -> Result<Option<LigandPoseBatchHandle>, CoreError> {
        if self.structure(owner).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let Some(instance_count) = template.instances_per_pose().checked_mul(poses.len()) else {
            return Err(invalid("licorice pose batch is too large"));
        };
        if u32::try_from(instance_count).is_err() {
            return Err(invalid("licorice pose batch exceeds the GPU draw range"));
        }
        if poses
            .iter()
            .any(|pose| !pose.transforms_finitely(template.bound_radius()))
        {
            return Err(invalid("licorice pose transform overflows model space"));
        }
        if instance_count == 0 {
            return Ok(None);
        }
        let handle = LigandPoseBatchHandle(
            self.ligand_pose_batches
                .insert(LigandPoseBatch::new(owner, template, poses)),
        );
        if handle.row() > crate::EntityId::MAX_INDEX {
            let _ = self.ligand_pose_batches.remove(handle.0);
            return Err(invalid("too many licorice pose batches for picking"));
        }
        self.primitive_revision = self.primitive_revision.wrapping_add(1);
        Ok(Some(handle))
    }

    /// Resolves one batch, rejecting stale generations in constant time.
    #[must_use]
    pub fn ligand_pose_batch(&self, handle: LigandPoseBatchHandle) -> Option<&LigandPoseBatch> {
        self.ligand_pose_batches.get(handle.0)
    }

    /// Hides one batch while keeping its topology, poses and stable handle.
    pub fn hide_ligand_pose_batch(&mut self, handle: LigandPoseBatchHandle) -> bool {
        self.set_ligand_pose_batch_visibility(handle, false)
    }

    /// Shows a previously hidden batch.
    pub fn show_ligand_pose_batch(&mut self, handle: LigandPoseBatchHandle) -> bool {
        self.set_ligand_pose_batch_visibility(handle, true)
    }

    /// Removes one batch and invalidates its handle.
    pub fn remove_ligand_pose_batch(
        &mut self,
        handle: LigandPoseBatchHandle,
    ) -> Option<LigandPoseBatch> {
        let removed = self.ligand_pose_batches.remove(handle.0);
        if removed.is_some() {
            self.primitive_revision = self.primitive_revision.wrapping_add(1);
        }
        removed
    }

    /// Iterates compact batches in stable slot order for GPU upload.
    #[doc(hidden)]
    pub fn ligand_pose_batches(
        &self,
    ) -> impl Iterator<Item = (LigandPoseBatchHandle, &LigandPoseBatch)> + '_ {
        self.ligand_pose_batches
            .iter()
            .map(|(raw, batch)| (LigandPoseBatchHandle(raw), batch))
    }

    /// Batch-level row written by every generated sphere and capsule.
    #[doc(hidden)]
    #[must_use]
    pub const fn ligand_pose_batch_entity_row(handle: LigandPoseBatchHandle) -> u32 {
        handle.row()
    }

    /// Resolves a picked generated instance to its compact source batch.
    #[must_use]
    pub fn ligand_pose_batch_for_entity(&self, entity: EntityRef) -> Option<&LigandPoseBatch> {
        if entity.kind != EntityKind::LigandPoseBatch {
            return None;
        }
        let (_, batch) = self.ligand_pose_batches.get_index(entity.index)?;
        (batch.owner() == entity.structure).then_some(batch)
    }

    fn set_ligand_pose_batch_visibility(
        &mut self,
        handle: LigandPoseBatchHandle,
        visible: bool,
    ) -> bool {
        let Some(batch) = self.ligand_pose_batches.get_mut(handle.0) else {
            return false;
        };
        if !batch.set_visible(visible) {
            return false;
        }
        self.primitive_revision = self.primitive_revision.wrapping_add(1);
        true
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidPrimitive { reason }
}

#[cfg(test)]
#[path = "ligand_pose_batches_tests.rs"]
mod tests;
