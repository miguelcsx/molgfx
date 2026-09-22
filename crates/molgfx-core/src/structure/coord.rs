//! The borrowed coordinate seam.
//!
//! The scene never owns coordinates. A `CoordRef` holds the parsed structure
//! (a cheap reference-counted handle) and the model it reads, and re-derives
//! the borrow on every access, so nothing is self-referential and the backing
//! buffer outlives every reader. The slice it returns is the parser's own
//! 64-byte-aligned storage; uploading it is a byte-cast, not a repack.

use molgfx_math::Aabb;

#[cfg(test)]
#[path = "coord_tests.rs"]
mod tests;

/// A borrowed view of one model's coordinate column.
#[derive(Clone, Debug)]
pub struct CoordRef {
    source: crate::MolecularSource,
    model: molframe::ModelIndex,
}

impl CoordRef {
    /// Borrows the given model's coordinates; `None` when the model does not
    /// exist or stores no dense coordinate block.
    #[must_use]
    pub fn new(structure: &molframe::Structure, model: molframe::ModelIndex) -> Option<Self> {
        structure.model_coordinates(model)?;
        Some(Self {
            source: crate::MolecularSource::from_molframe(structure),
            model,
        })
    }

    /// Borrows the provider's canonical coordinate column.
    #[must_use]
    pub fn from_source(source: &crate::MolecularSource) -> Self {
        Self {
            source: source.clone(),
            model: molframe::ModelIndex::new(0),
        }
    }

    /// The coordinate slice, straight from the parser's storage. `O(1)`.
    #[must_use]
    pub fn slice(&self) -> &[[f32; 3]] {
        self.source.coordinates()
    }

    /// The coordinates as raw bytes for direct upload, 12 bytes per atom.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(self.slice())
    }

    /// Number of atoms in the column.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slice().len()
    }

    /// True when the column has no atoms.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slice().is_empty()
    }

    /// The coordinate generation; changes exactly when coordinates change,
    /// keying spatial caches such as the bounding hierarchy.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.source.coordinate_revision()
    }

    /// Which model this reference reads.
    #[must_use]
    pub fn model(&self) -> molframe::ModelIndex {
        self.model
    }

    /// The tightest bound over the column, `O(n)`; skips non-finite rows.
    #[must_use]
    pub fn aabb(&self) -> Aabb {
        Aabb::from_points(self.slice().iter().map(|p| (*p).into()))
    }
}
