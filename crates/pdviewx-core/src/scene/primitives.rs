//! Revisioned storage for caller-authored analytic primitives.

use crate::{
    AnisotropicEllipsoid, CarbohydrateSymbol, CoreError, Particle, PlanarRegion, Primitive,
    PrimitiveHandle, Scene, StructureHandle,
};
use pdviewx_math::Rgba8;

impl Scene {
    /// Stores a validated ellipsoid owned by one placed structure.
    ///
    /// The tensor is kept in model space and transformed once when the GPU
    /// table is packed; it is never approximated by a mesh on the CPU.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when `owner` does not identify a
    /// structure in this scene, or [`CoreError::InvalidPrimitive`]
    /// when `opacity` is not finite or is outside `[0, 1]`.
    pub fn add_ellipsoid(
        &mut self,
        owner: StructureHandle,
        value: AnisotropicEllipsoid,
        color: Rgba8,
        opacity: f32,
    ) -> Result<PrimitiveHandle, CoreError> {
        if self.structure(owner).is_none() || !valid_opacity(opacity) {
            return Err(if self.structure(owner).is_none() {
                CoreError::StaleHandle
            } else {
                CoreError::InvalidPrimitive {
                    reason: "primitive opacity must be finite in [0, 1]",
                }
            });
        }
        Ok(self.insert_primitive(Primitive::Ellipsoid {
            owner,
            value,
            color,
            opacity,
            visible: true,
        }))
    }

    /// Stores a caller-resolved carbohydrate symbol.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the symbol's owner does not
    /// identify a structure in this scene.
    pub fn add_carbohydrate_symbol(
        &mut self,
        value: CarbohydrateSymbol,
    ) -> Result<PrimitiveHandle, CoreError> {
        if self.structure(value.owner).is_none() {
            return Err(CoreError::StaleHandle);
        }
        Ok(self.insert_primitive(Primitive::Carbohydrate(value)))
    }

    /// Stores a filled rectangular primitive plane.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the plane's owner does not
    /// identify a structure in this scene, or
    /// [`CoreError::InvalidPrimitive`] when `opacity` is not finite
    /// or is outside `[0, 1]`.
    pub fn add_filled_planar_region(
        &mut self,
        value: PlanarRegion,
        color: Rgba8,
        opacity: f32,
    ) -> Result<PrimitiveHandle, CoreError> {
        if self.structure(value.owner).is_none() || !valid_opacity(opacity) {
            return Err(if self.structure(value.owner).is_none() {
                CoreError::StaleHandle
            } else {
                CoreError::InvalidPrimitive {
                    reason: "primitive opacity must be finite in [0, 1]",
                }
            });
        }
        Ok(self.insert_primitive(Primitive::Planar {
            value,
            color,
            opacity,
            visible: true,
        }))
    }

    /// Stores one generic caller-authored particle in the shared analytic table.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the particle owner is absent.
    pub fn add_particle(&mut self, value: Particle) -> Result<PrimitiveHandle, CoreError> {
        if self.structure(value.owner).is_none() {
            return Err(CoreError::StaleHandle);
        }
        Ok(self.insert_primitive(Primitive::Particle(value)))
    }

    /// Stores a batch of generic particles in caller input order. The returned
    /// handles retain that order and share the same heterogeneous GPU table.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] without inserting anything when one
    /// particle owner is absent.
    pub fn add_particles(
        &mut self,
        values: &[Particle],
    ) -> Result<Vec<PrimitiveHandle>, CoreError> {
        if values
            .iter()
            .any(|value| self.structure(value.owner).is_none())
        {
            return Err(CoreError::StaleHandle);
        }
        let mut handles = Vec::with_capacity(values.len());
        for value in values.iter().copied() {
            handles.push(self.insert_primitive(Primitive::Particle(value)));
        }
        Ok(handles)
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

    fn insert_primitive(&mut self, value: Primitive) -> PrimitiveHandle {
        self.primitive_revision = self.primitive_revision.wrapping_add(1);
        PrimitiveHandle(self.primitive.insert(value))
    }
}

const fn valid_opacity(opacity: f32) -> bool {
    opacity.is_finite() && opacity >= 0.0 && opacity <= 1.0
}
