//! Where a hierarchy reads its primitive bounds from.
//!
//! Building over a `&[Aabb]` forces the caller to materialise one box per
//! primitive first. At twenty-four bytes each that is a temporary the size of
//! the structure — twenty-four gigabytes for a billion atoms — allocated only
//! to be read once and dropped. A source computes each box on demand from the
//! columns the scene already owns, so the temporary never exists.
//!
//! Implementations must be pure and cheap: the builder reads every bound twice
//! (once for the scene bound, once for the leaf it lands in) and reads them
//! from several threads at once.

use crate::{Aabb, Vec3};

/// A random-access supply of primitive bounds.
pub trait BvhSource: Sync {
    /// Number of primitives addressable by [`Self::bound`].
    fn len(&self) -> usize;

    /// Bound of one primitive. Rows outside `0..len()` return the empty bound,
    /// which the builder skips.
    fn bound(&self, index: u32) -> Aabb;

    /// Whether the source supplies no primitives.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl BvhSource for [Aabb] {
    #[inline]
    fn len(&self) -> usize {
        <[Aabb]>::len(self)
    }

    #[inline]
    fn bound(&self, index: u32) -> Aabb {
        match self.get(index as usize) {
            Some(bound) => *bound,
            None => Aabb::EMPTY,
        }
    }
}

impl BvhSource for Vec<Aabb> {
    #[inline]
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    #[inline]
    fn bound(&self, index: u32) -> Aabb {
        self.as_slice().bound(index)
    }
}

impl<const N: usize> BvhSource for [Aabb; N] {
    #[inline]
    fn len(&self) -> usize {
        N
    }

    #[inline]
    fn bound(&self, index: u32) -> Aabb {
        self.as_slice().bound(index)
    }
}

impl<T: BvhSource + ?Sized> BvhSource for &T {
    #[inline]
    fn len(&self) -> usize {
        (**self).len()
    }

    #[inline]
    fn bound(&self, index: u32) -> Aabb {
        (**self).bound(index)
    }
}

/// Bounds derived from a position column and a radius column.
///
/// This is the atom case, and the reason the trait exists: a scene already
/// holds both columns, so the boxes are arithmetic rather than storage. Rows
/// past the shorter of the two columns are empty.
#[derive(Clone, Copy, Debug)]
pub struct SphereBounds<'a> {
    centers: &'a [[f32; 3]],
    radii: &'a [f32],
}

impl<'a> SphereBounds<'a> {
    /// Pairs a position column with a radius column.
    #[must_use]
    pub fn new(centers: &'a [[f32; 3]], radii: &'a [f32]) -> Self {
        Self { centers, radii }
    }
}

impl BvhSource for SphereBounds<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.centers.len().min(self.radii.len())
    }

    #[inline]
    fn bound(&self, index: u32) -> Aabb {
        let index = index as usize;
        let (Some(center), Some(radius)) = (self.centers.get(index), self.radii.get(index)) else {
            return Aabb::EMPTY;
        };
        let center = Vec3::from_array(*center);
        let extent = Vec3::splat(radius.abs());
        Aabb::new(center - extent, center + extent)
    }
}

/// Bounds swept between two position columns sharing one radius column.
///
/// A trajectory interpolates between two frames, so the primitive a ray can hit
/// is the box enclosing both endpoints. Building over this keeps the moving
/// hierarchy conservative without materialising the swept boxes.
#[derive(Clone, Copy, Debug)]
pub struct SweptSphereBounds<'a> {
    start: &'a [[f32; 3]],
    end: &'a [[f32; 3]],
    radii: &'a [f32],
}

impl<'a> SweptSphereBounds<'a> {
    /// Pairs two position columns with the radius column they share.
    #[must_use]
    pub fn new(start: &'a [[f32; 3]], end: &'a [[f32; 3]], radii: &'a [f32]) -> Self {
        Self { start, end, radii }
    }
}

impl BvhSource for SweptSphereBounds<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.start.len().min(self.end.len()).min(self.radii.len())
    }

    #[inline]
    fn bound(&self, index: u32) -> Aabb {
        let index = index as usize;
        let (Some(start), Some(end), Some(radius)) = (
            self.start.get(index),
            self.end.get(index),
            self.radii.get(index),
        ) else {
            return Aabb::EMPTY;
        };
        let start = Vec3::from_array(*start);
        let end = Vec3::from_array(*end);
        let extent = Vec3::splat(radius.abs());
        Aabb::new(start.min(end) - extent, start.max(end) + extent)
    }
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
