//! Stable scene storage for immutable caller-supplied atom properties.

use crate::scene::StoredAtomProperty;
use crate::{AtomProperty, AtomPropertyHandle, CoreError, Scene, StructureHandle};

#[cfg(test)]
#[path = "properties_tests.rs"]
mod tests;

impl Scene {
    /// Adds one immutable atom property after validating its owner and length.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed owner and
    /// [`CoreError::InvalidProperty`] when the column length differs from the
    /// owner's atom count.
    pub fn add_atom_property(
        &mut self,
        property: AtomProperty,
    ) -> Result<AtomPropertyHandle, CoreError> {
        validate(self, &property)?;
        self.property_revision = self.property_revision.wrapping_add(1);
        Ok(AtomPropertyHandle(self.properties.insert(
            StoredAtomProperty {
                value: property,
                revision: self.property_revision,
            },
        )))
    }

    /// Resolves a property handle.
    #[must_use]
    pub fn atom_property(&self, handle: AtomPropertyHandle) -> Option<&AtomProperty> {
        self.properties.get(handle.0).map(|stored| &stored.value)
    }

    /// Replaces a property while retaining the handle used by representations.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale property/owner or mismatched length.
    pub fn replace_atom_property(
        &mut self,
        handle: AtomPropertyHandle,
        property: AtomProperty,
    ) -> Result<(), CoreError> {
        if self.properties.get(handle.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        validate(self, &property)?;
        self.property_revision = self.property_revision.wrapping_add(1);
        let stored = self
            .properties
            .get_mut(handle.0)
            .ok_or(CoreError::StaleHandle)?;
        stored.value = property;
        stored.revision = self.property_revision;
        Ok(())
    }

    /// Frame-synced update of one scalar column from its two bracketing rows.
    ///
    /// This is the scalar analogue of the two-frame trajectory seam: the scene
    /// never stores a whole time series. A caller with a per-frame table (RMSF,
    /// interaction energy, per-residue affinity) passes only the two rows that
    /// bracket the current time plus the same interpolation fraction used for
    /// coordinates, and the column is replaced in place under the same handle,
    /// name, meaning and legend — so bound representations recolour frame by
    /// frame without rebinding or a second upload path. `NaN` in either row is
    /// a missing value and yields `NaN`, matching the column's missing-value
    /// contract.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an unknown handle,
    /// [`CoreError::InvalidProperty`] for a non-finite `alpha` outside `[0, 1]`,
    /// unequal row lengths, or a fully missing result, and propagates the
    /// length check against the owning structure.
    pub fn interpolate_atom_property(
        &mut self,
        handle: AtomPropertyHandle,
        start: &[f32],
        end: &[f32],
        alpha: f32,
    ) -> Result<(), CoreError> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(CoreError::InvalidProperty {
                reason: "property interpolation fraction must be finite in [0, 1]",
            });
        }
        if start.len() != end.len() {
            return Err(CoreError::InvalidProperty {
                reason: "property frame rows must have equal length",
            });
        }
        let existing = self.atom_property(handle).ok_or(CoreError::StaleHandle)?;
        if start.len() != existing.values().len() {
            return Err(CoreError::InvalidProperty {
                reason: "property frame length must match its owning structure",
            });
        }
        let stored = self
            .properties
            .get_mut(handle.0)
            .ok_or(CoreError::StaleHandle)?;
        stored.value.interpolate_values(start, end, alpha)?;
        self.property_revision = self.property_revision.wrapping_add(1);
        stored.revision = self.property_revision;
        Ok(())
    }

    /// Removes one property and invalidates its handle.
    pub fn remove_atom_property(&mut self, handle: AtomPropertyHandle) -> Option<AtomProperty> {
        let removed = self.properties.remove(handle.0).map(|stored| stored.value);
        if removed.is_some() {
            self.property_revision = self.property_revision.wrapping_add(1);
        }
        removed
    }

    /// Iterates properties in stable scene-table order.
    pub fn atom_properties(
        &self,
    ) -> impl Iterator<Item = (AtomPropertyHandle, &AtomProperty)> + '_ {
        self.properties
            .iter()
            .map(|(handle, stored)| (AtomPropertyHandle(handle), &stored.value))
    }

    /// Revision key for property membership/content.
    #[must_use]
    pub const fn property_revision(&self) -> u64 {
        self.property_revision
    }

    /// Content revision of one live property column.
    #[must_use]
    pub fn property_content_revision(&self, handle: AtomPropertyHandle) -> Option<u64> {
        self.properties.get(handle.0).map(|stored| stored.revision)
    }

    /// Resolves a property only when it belongs to `structure`.
    #[must_use]
    pub fn property_for_structure(
        &self,
        property: AtomPropertyHandle,
        structure: StructureHandle,
    ) -> Option<&AtomProperty> {
        self.atom_property(property)
            .filter(|value| value.owner() == structure)
    }
}

fn validate(scene: &Scene, property: &AtomProperty) -> Result<(), CoreError> {
    let structure = scene
        .structure(property.owner())
        .ok_or(CoreError::StaleHandle)?;
    let expected = usize::try_from(structure.atoms.len()).map_or(usize::MAX, |value| value);
    if property.values().len() != expected {
        return Err(CoreError::InvalidProperty {
            reason: "atom property length must equal the owning structure atom count",
        });
    }
    Ok(())
}
