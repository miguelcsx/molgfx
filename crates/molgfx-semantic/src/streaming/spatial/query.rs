//! Shared conservative traversal for culling, picking and quality consumers.

use super::{PagedSpatialIndex, SpatialCandidate, SpatialError};
use molgfx_core::{LocalRow, LogicalRow};
use molgfx_math::{Aabb, Vec3};

#[derive(Clone, Copy)]
enum Query {
    Aabb(Aabb),
    Sphere { center: Vec3, radius: f32 },
    Ray { origin: Vec3, direction: Vec3 },
}

impl PagedSpatialIndex {
    /// Appends candidates overlapping an axis-aligned culling region.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::UncommittedChanges`] until staged changes are committed.
    pub fn aabb_candidates(
        &mut self,
        query: Aabb,
        output: &mut Vec<SpatialCandidate>,
    ) -> Result<(), SpatialError> {
        self.query(Query::Aabb(query), output)
    }

    /// Appends candidates near a quality or neighbourhood sample sphere.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::UncommittedChanges`] until staged changes are committed.
    pub fn sphere_candidates(
        &mut self,
        center: Vec3,
        radius: f32,
        output: &mut Vec<SpatialCandidate>,
    ) -> Result<(), SpatialError> {
        self.query(Query::Sphere { center, radius }, output)
    }

    /// Appends candidates along a picking or quality ray.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::UncommittedChanges`] until staged changes are committed.
    pub fn ray_candidates(
        &mut self,
        origin: Vec3,
        direction: Vec3,
        output: &mut Vec<SpatialCandidate>,
    ) -> Result<(), SpatialError> {
        self.query(Query::Ray { origin, direction }, output)
    }

    fn query(
        &mut self,
        query: Query,
        output: &mut Vec<SpatialCandidate>,
    ) -> Result<(), SpatialError> {
        self.ensure_committed()?;
        output.clear();
        match query {
            Query::Aabb(bound) => self.tlas.aabb_candidates(
                bound,
                &mut self.query_scratch.tlas_walk,
                &mut self.query_scratch.pages,
            ),
            Query::Sphere { center, radius } => self.tlas.sphere_candidates(
                center,
                radius,
                &mut self.query_scratch.tlas_walk,
                &mut self.query_scratch.pages,
            ),
            Query::Ray { origin, direction } => self.tlas.ray_candidates(
                origin,
                direction,
                &mut self.query_scratch.tlas_walk,
                &mut self.query_scratch.pages,
            ),
        }
        for page in self.query_scratch.pages.iter().copied() {
            let Some(slot) = self.pages.get(page as usize) else {
                continue;
            };
            let Some(chunk) = slot.chunk else {
                continue;
            };
            match query {
                Query::Aabb(bound) => slot.hierarchy.aabb_candidates(
                    bound,
                    &mut self.query_scratch.blas_walk,
                    &mut self.query_scratch.primitives,
                ),
                Query::Sphere { center, radius } => slot.hierarchy.sphere_candidates(
                    center,
                    radius,
                    &mut self.query_scratch.blas_walk,
                    &mut self.query_scratch.primitives,
                ),
                Query::Ray { origin, direction } => slot.hierarchy.ray_candidates(
                    origin,
                    direction,
                    &mut self.query_scratch.blas_walk,
                    &mut self.query_scratch.primitives,
                ),
            }
            for local in self.query_scratch.primitives.iter().copied() {
                output.push(SpatialCandidate {
                    dataset: chunk.dataset,
                    chunk: chunk.chunk,
                    row: LogicalRow::new(chunk.rows.first().get() + u64::from(local)),
                    local_row: LocalRow::new(local),
                    page,
                });
            }
        }
        Ok(())
    }
}
