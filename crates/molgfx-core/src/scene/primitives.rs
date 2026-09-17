//! Revisioned storage for caller-authored analytic primitives.

use crate::{CoreError, Primitive, PrimitiveHandle, Scene};

impl Scene {
    /// Inserts one validated native batch into the shared analytic table.
    ///
    /// Validation is atomic, storage is reserved once and the scene revision
    /// advances once, regardless of item count. This is the public path for
    /// high-cardinality caller geometry.
    ///
    /// # Errors
    ///
    /// Returns without inserting anything when an owner is stale or an alpha
    /// value is malformed.
    pub fn add_primitives(
        &mut self,
        values: &[Primitive],
    ) -> Result<Option<PrimitiveHandle>, CoreError> {
        for value in values {
            self.validate_primitive(*value)?;
        }
        self.primitive.reserve(values.len());
        let mut last = None;
        for value in values {
            last = Some(PrimitiveHandle(self.primitive.insert(*value)));
        }
        if !values.is_empty() {
            self.primitive_revision = self.primitive_revision.wrapping_add(1);
        }
        Ok(last)
    }

    /// Resolves one primitive.
    #[must_use]
    pub fn primitive(&self, handle: PrimitiveHandle) -> Option<&Primitive> {
        self.primitive.get(handle.0)
    }

    /// Mutable access; edits invalidate the one shared GPU table.
    pub fn primitive_mut(&mut self, handle: PrimitiveHandle) -> Option<&mut Primitive> {
        let value = self.primitive.get_mut(handle.0)?;
        self.primitive_revision = self.primitive_revision.wrapping_add(1);
        Some(value)
    }

    /// Removes a primitive and invalidates its handle.
    pub fn remove_primitive(&mut self, handle: PrimitiveHandle) -> Option<Primitive> {
        let value = self.primitive.remove(handle.0)?;
        self.primitive_revision = self.primitive_revision.wrapping_add(1);
        Some(value)
    }

    /// Iterates active and hidden primitives in stable slot order.
    pub fn primitives(&self) -> impl Iterator<Item = (PrimitiveHandle, &Primitive)> + '_ {
        self.primitive
            .iter()
            .map(|(handle, value)| (PrimitiveHandle(handle), value))
    }

    /// Revision key for the heterogeneous analytic primitive table.
    #[must_use]
    pub const fn primitive_revision(&self) -> u64 {
        self.primitive_revision
    }

    /// Stable row used by primitive picking.
    #[must_use]
    pub const fn primitive_row(handle: PrimitiveHandle) -> u32 {
        handle.row()
    }

    /// Resolves a picked primitive row without scanning the table.
    #[must_use]
    pub fn primitive_for_entity(&self, entity: crate::EntityRef) -> Option<&Primitive> {
        if entity.kind != crate::EntityKind::Primitive {
            return None;
        }
        self.primitive
            .get_index(entity.index)
            .map(|(_, value)| value)
    }

    fn validate_primitive(&self, value: Primitive) -> Result<(), CoreError> {
        if self.structure(value.owner()).is_none() {
            return Err(CoreError::StaleHandle);
        }
        let opacity = match value {
            Primitive::Ellipsoid { opacity, .. } | Primitive::Planar { opacity, .. } => opacity,
            Primitive::Particle(value) => value.opacity,
            Primitive::Carbohydrate(_) => return Ok(()),
        };
        if valid_opacity(opacity) {
            Ok(())
        } else {
            Err(CoreError::InvalidPrimitive {
                reason: "primitive opacity must be finite in [0, 1]",
            })
        }
    }
}

const fn valid_opacity(opacity: f32) -> bool {
    opacity.is_finite() && opacity >= 0.0 && opacity <= 1.0
}
