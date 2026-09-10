//! Atomic scene insertion for generic points, instances and relations.

#[cfg(test)]
#[path = "generic_batches_tests.rs"]
mod tests;

use crate::{
    CoreError, EntityKind, GlobalPickIdentity, InstanceBatch, InstanceBatchHandle, PointBatch,
    PointBatchHandle, RelationBatch, RelationBatchHandle, RowDomain, Scene, TemplatePartPick,
    TemplatePartRef,
};

impl Scene {
    /// Inserts one prevalidated point batch in constant time.
    #[must_use]
    pub fn add_point_batch(&mut self, batch: PointBatch) -> PointBatchHandle {
        let handle = PointBatchHandle(self.point_batches.insert(batch));
        self.generic_batch_revision = self.generic_batch_revision.wrapping_add(1);
        handle
    }

    /// Resolves a point batch by generational handle.
    #[must_use]
    pub fn point_batch(&self, handle: PointBatchHandle) -> Option<&PointBatch> {
        self.point_batches.get(handle.0)
    }

    /// Iterates point batches in stable slot order.
    pub fn point_batches(&self) -> impl Iterator<Item = (PointBatchHandle, &PointBatch)> + '_ {
        self.point_batches
            .iter()
            .map(|(handle, batch)| (PointBatchHandle(handle), batch))
    }

    /// Removes one point batch and invalidates its domain.
    pub fn remove_point_batch(&mut self, handle: PointBatchHandle) -> Option<PointBatch> {
        let removed = self.point_batches.remove(handle.0);
        if removed.is_some() {
            self.unbind_point_frames(handle);
            let _descriptor = self.remove_domain_visual(RowDomain::Points(handle));
            self.generic_batch_revision = self.generic_batch_revision.wrapping_add(1);
        }
        removed
    }

    /// Inserts one prevalidated shared-template instance batch in constant time.
    #[must_use]
    pub fn add_instance_batch(&mut self, batch: InstanceBatch) -> InstanceBatchHandle {
        let handle = InstanceBatchHandle(self.instance_batches.insert(batch));
        self.generic_batch_revision = self.generic_batch_revision.wrapping_add(1);
        handle
    }

    /// Resolves an instance batch by generational handle.
    #[must_use]
    pub fn instance_batch(&self, handle: InstanceBatchHandle) -> Option<&InstanceBatch> {
        self.instance_batches.get(handle.0)
    }

    /// Iterates instance batches in stable slot order.
    pub fn instance_batches(
        &self,
    ) -> impl Iterator<Item = (InstanceBatchHandle, &InstanceBatch)> + '_ {
        self.instance_batches
            .iter()
            .map(|(handle, batch)| (InstanceBatchHandle(handle), batch))
    }

    /// Removes one instance batch and invalidates instance/template domains.
    pub fn remove_instance_batch(&mut self, handle: InstanceBatchHandle) -> Option<InstanceBatch> {
        let removed = self.instance_batches.remove(handle.0);
        if removed.is_some() {
            self.unbind_instance_frames(handle);
            let _instance = self.remove_domain_visual(RowDomain::Instances(handle));
            let _parts = self.remove_domain_visual(RowDomain::TemplateParts(handle));
            self.generic_batch_revision = self.generic_batch_revision.wrapping_add(1);
        }
        removed
    }

    /// Inserts a prepartitioned relation batch after validating only its
    /// deduplicated domain dependencies.
    ///
    /// # Errors
    ///
    /// A stale domain or row mismatch rejects the whole insertion atomically.
    pub fn add_relation_batch(
        &mut self,
        batch: RelationBatch,
    ) -> Result<RelationBatchHandle, CoreError> {
        for dependency in batch.dependencies().iter() {
            let rows = self
                .row_count(dependency.domain)
                .ok_or(CoreError::StaleHandle)?;
            if dependency.maximum_row >= rows {
                return Err(CoreError::InvalidBatch {
                    reason: "relation anchor row is outside its live domain",
                });
            }
        }
        let handle = RelationBatchHandle(self.relation_batches.insert(batch));
        self.generic_batch_revision = self.generic_batch_revision.wrapping_add(1);
        Ok(handle)
    }

    /// Resolves a relation batch by generational handle.
    #[must_use]
    pub fn relation_batch(&self, handle: RelationBatchHandle) -> Option<&RelationBatch> {
        self.relation_batches.get(handle.0)
    }

    /// Iterates relation batches in stable slot order.
    pub fn relation_batches(
        &self,
    ) -> impl Iterator<Item = (RelationBatchHandle, &RelationBatch)> + '_ {
        self.relation_batches
            .iter()
            .map(|(handle, batch)| (RelationBatchHandle(handle), batch))
    }

    /// Removes one relation batch and invalidates its row domain.
    pub fn remove_relation_batch(&mut self, handle: RelationBatchHandle) -> Option<RelationBatch> {
        let removed = self.relation_batches.remove(handle.0);
        if removed.is_some() {
            let _descriptor = self.remove_domain_visual(RowDomain::Relations(handle));
            self.generic_batch_revision = self.generic_batch_revision.wrapping_add(1);
        }
        removed
    }

    /// Resolves the exact live row count for a generic domain in `O(1)`.
    #[must_use]
    pub fn row_count(&self, domain: RowDomain) -> Option<u32> {
        match domain {
            RowDomain::Atoms(handle) => Some(self.structure(handle)?.atoms.len()),
            RowDomain::Points(handle) => Some(self.point_batch(handle)?.source_rows().len()),
            RowDomain::Instances(handle) => Some(self.instance_batch(handle)?.source_rows().len()),
            RowDomain::TemplateParts(handle) => {
                u32::try_from(self.instance_batch(handle)?.template().part_count()).ok()
            }
            RowDomain::Relations(handle) => Some(self.relation_batch(handle)?.source_rows().len()),
        }
    }

    /// Revision shared by generic batch membership and visibility.
    #[must_use]
    pub const fn generic_batch_revision(&self) -> u64 {
        self.generic_batch_revision
    }

    /// Resolves a flattened template-part pick by quotient/remainder without
    /// adding another attachment or GPU lookup table.
    #[must_use]
    pub fn resolve_template_part_pick(
        &self,
        identity: GlobalPickIdentity,
    ) -> Option<TemplatePartPick> {
        if identity.kind() != EntityKind::TemplatePart {
            return None;
        }
        let chunk = identity.chunk().get();
        let slot = u32::try_from(chunk & u64::from(u32::MAX)).ok()?;
        let generation = u32::try_from(chunk >> 32).ok()?;
        let (handle, batch) = self
            .instance_batches()
            .find(|(handle, _)| handle.row() == slot && handle.generation() == generation)?;
        if identity.dataset().get() != batch.template().source_rows().namespace().0 {
            return None;
        }
        let part_count = u64::try_from(batch.template().part_count()).ok()?;
        let flat = identity.row().get();
        let instance_row = u32::try_from(flat / part_count).ok()?;
        let part_row = u32::try_from(flat % part_count).ok()?;
        let instance_source_key = batch.source_rows().key(instance_row)?;
        let part_source_key = batch.template().source_rows().key(part_row)?;
        Some(TemplatePartPick::new(
            TemplatePartRef::new(handle, instance_row, part_row),
            instance_source_key,
            part_source_key,
        ))
    }

    /// Changes generic batch visibility without touching payload storage.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a dead domain and rejects atom
    /// domains because their visibility is representation-owned.
    pub fn set_domain_visible(
        &mut self,
        domain: RowDomain,
        visible: bool,
    ) -> Result<bool, CoreError> {
        let changed = match domain {
            RowDomain::Points(handle) => self
                .point_batches
                .get_mut(handle.0)
                .ok_or(CoreError::StaleHandle)?
                .set_visible(visible),
            RowDomain::Instances(handle) | RowDomain::TemplateParts(handle) => self
                .instance_batches
                .get_mut(handle.0)
                .ok_or(CoreError::StaleHandle)?
                .set_visible(visible),
            RowDomain::Relations(handle) => self
                .relation_batches
                .get_mut(handle.0)
                .ok_or(CoreError::StaleHandle)?
                .set_visible(visible),
            RowDomain::Atoms(_) => {
                return Err(CoreError::InvalidBatch {
                    reason: "atom visibility is controlled by representations and selections",
                });
            }
        };
        if changed {
            self.generic_batch_revision = self.generic_batch_revision.wrapping_add(1);
        }
        Ok(changed)
    }
}
