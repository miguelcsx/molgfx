//! Stable scene storage for typed immutable attribute columns.

#[cfg(test)]
#[path = "attributes_tests.rs"]
mod tests;

use crate::scene::StoredAttribute;
use crate::{AttributeColumn, AttributeHandle, CoreError, RowDomain, Scene};
use std::ops::Range;

impl Scene {
    /// Inserts a source-aligned immutable column after an `O(1)` domain check.
    ///
    /// # Errors
    ///
    /// The target must be live and have exactly the same row count.
    pub fn add_attribute(
        &mut self,
        attribute: AttributeColumn,
    ) -> Result<AttributeHandle, CoreError> {
        validate(self, &attribute)?;
        let end = u32::try_from(attribute.len()).map_err(|_| invalid("attribute exceeds u32"))?;
        self.attribute_revision = self.attribute_revision.wrapping_add(1);
        Ok(AttributeHandle(self.attributes.insert(StoredAttribute {
            value: attribute,
            revision: self.attribute_revision,
            dirty_rows: 0..end,
        })))
    }

    /// Resolves one typed column.
    #[must_use]
    pub fn attribute(&self, handle: AttributeHandle) -> Option<&AttributeColumn> {
        self.attributes.get(handle.0).map(|stored| &stored.value)
    }

    /// Atomically swaps immutable backing and records the only changed range.
    ///
    /// The caller prepares the new `Arc` outside the scene. GPU synchronization
    /// uploads only `dirty_rows`; unchanged rows preserve residency.
    ///
    /// # Errors
    ///
    /// Handle, domain, row count and dirty range are validated before mutation.
    pub fn replace_attribute_range(
        &mut self,
        handle: AttributeHandle,
        attribute: AttributeColumn,
        dirty_rows: Range<u32>,
    ) -> Result<(), CoreError> {
        let previous = self.attribute(handle).ok_or(CoreError::StaleHandle)?;
        if previous.domain() != attribute.domain() || previous.kind() != attribute.kind() {
            return Err(invalid(
                "attribute replacement must preserve target domain and physical kind",
            ));
        }
        validate(self, &attribute)?;
        let end = u32::try_from(attribute.len()).map_err(|_| invalid("attribute exceeds u32"))?;
        if dirty_rows.start > dirty_rows.end || dirty_rows.end > end {
            return Err(invalid("attribute dirty range is outside the column"));
        }
        self.attribute_revision = self.attribute_revision.wrapping_add(1);
        let stored = self
            .attributes
            .get_mut(handle.0)
            .ok_or(CoreError::StaleHandle)?;
        stored.value = attribute;
        stored.revision = self.attribute_revision;
        stored.dirty_rows = dirty_rows;
        Ok(())
    }

    /// Removes one column and invalidates its handle.
    pub fn remove_attribute(&mut self, handle: AttributeHandle) -> Option<AttributeColumn> {
        let removed = self.attributes.remove(handle.0).map(|stored| stored.value);
        if removed.is_some() {
            self.unbind_attribute_frames(handle);
            self.attribute_revision = self.attribute_revision.wrapping_add(1);
        }
        removed
    }

    /// Iterates columns in stable slot order for one deduplicated GPU arena.
    pub fn attributes(&self) -> impl Iterator<Item = (AttributeHandle, &AttributeColumn)> + '_ {
        self.attributes
            .iter()
            .map(|(handle, stored)| (AttributeHandle(handle), &stored.value))
    }

    /// Scene-wide membership/content revision.
    #[must_use]
    pub const fn attribute_revision(&self) -> u64 {
        self.attribute_revision
    }

    /// Content revision and changed row range for one live column.
    #[must_use]
    pub fn attribute_change(&self, handle: AttributeHandle) -> Option<(u64, Range<u32>)> {
        self.attributes
            .get(handle.0)
            .map(|stored| (stored.revision, stored.dirty_rows.clone()))
    }

    /// Resolves a column only when it targets the exact requested domain.
    #[must_use]
    pub fn attribute_for_domain(
        &self,
        handle: AttributeHandle,
        domain: RowDomain,
    ) -> Option<&AttributeColumn> {
        self.attribute(handle)
            .filter(|attribute| attribute.domain() == domain)
    }
}

fn validate(scene: &Scene, attribute: &AttributeColumn) -> Result<(), CoreError> {
    let expected = scene
        .row_count(attribute.domain())
        .ok_or(CoreError::StaleHandle)?;
    if attribute.len() != expected as usize {
        return Err(invalid("attribute length must equal its target row domain"));
    }
    Ok(())
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidAttribute { reason }
}
