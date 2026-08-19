//! A structure placed in a scene.
//!
//! Placement pairs the immutable parsed structure with a model transform and
//! the scene's per-atom tables. Multiple placements of the same structure
//! share its storage; each carries its own transform and columns.

use crate::atoms::AtomTable;
use crate::hierarchy::Hierarchy;
use crate::{Column, SecondaryStructure, TrajectorySegment};
use pdviewx_math::{Aabb, Bvh, Mat4, Vec3};
use std::sync::OnceLock;

/// One structure in the scene, with its transform and derived tables.
#[derive(Clone, Debug)]
pub struct PlacedStructure {
    /// The parsed structure; a cheap reference-counted handle.
    pub structure: pdbiox::Structure,
    /// Model-to-world transform.
    pub model_to_world: Mat4,
    /// The dense per-atom table for the placed model.
    pub atoms: AtomTable,
    /// Offset-array hierarchy over the structure's topology.
    pub hierarchy: Hierarchy,
    /// Shared atom hierarchy used by selection, culling, surfaces and picking.
    spatial_bvh: OnceLock<Bvh>,
    spatial_bounds: Aabb,
    /// Per-residue secondary structure supplied by the caller or `pdbiox`.
    pub secondary_structure: Column<SecondaryStructure>,
    trajectory: Option<TrajectorySegment>,
    trajectory_bvh: Option<Bvh>,
    trajectory_revision: u64,
    trajectory_pair_revision: u64,
}

impl PlacedStructure {
    /// Places the first model of a structure at the identity transform.
    #[must_use]
    pub fn new(structure: &pdbiox::Structure) -> Option<Self> {
        let model = pdbiox::ModelIndex::new(0);
        let atoms = AtomTable::from_structure(structure, model)?;
        let hierarchy = Hierarchy::from_structure(structure);
        let spatial_bounds = atom_bounds(&atoms);
        let secondary_structure =
            Column::new(vec![SecondaryStructure::Coil; hierarchy.residue_count()]);
        Some(Self {
            structure: structure.clone(),
            model_to_world: Mat4::IDENTITY,
            hierarchy,
            spatial_bvh: OnceLock::new(),
            spatial_bounds,
            secondary_structure,
            atoms,
            trajectory: None,
            trajectory_bvh: None,
            trajectory_revision: 0,
            trajectory_pair_revision: 0,
        })
    }

    /// World-space bound of the active coordinate interval, `O(1)`.
    #[must_use]
    pub fn world_aabb(&self) -> Aabb {
        self.trajectory_bvh
            .as_ref()
            .map_or(self.spatial_bounds, Bvh::bounds)
            .transform(&self.model_to_world)
    }

    /// Active caller-supplied interpolation interval, when present.
    #[must_use]
    pub const fn trajectory(&self) -> Option<&TrajectorySegment> {
        self.trajectory.as_ref()
    }

    /// Revision of the active trajectory pair or sample time.
    #[must_use]
    pub const fn trajectory_revision(&self) -> u64 {
        self.trajectory_revision
    }

    /// Revision of the resident frame pair, excluding time-only updates.
    #[must_use]
    pub const fn trajectory_pair_revision(&self) -> u64 {
        self.trajectory_pair_revision
    }

    /// Hierarchy conservatively enclosing every active interpolated position.
    #[must_use]
    pub fn render_bvh(&self) -> &Bvh {
        match &self.trajectory_bvh {
            Some(trajectory) => trajectory,
            None => self.spatial_bvh(),
        }
    }

    /// Lazily materializes the atom hierarchy only for spatial work.
    #[must_use]
    pub fn spatial_bvh(&self) -> &Bvh {
        self.spatial_bvh.get_or_init(|| atom_bvh(&self.atoms))
    }

    #[cfg(test)]
    pub(crate) fn spatial_bvh_is_ready(&self) -> bool {
        self.spatial_bvh.get().is_some()
    }

    pub(crate) fn replace_trajectory(&mut self, segment: TrajectorySegment) {
        self.trajectory_bvh = Some(trajectory_bvh(&segment, self.atoms.radius().values()));
        self.trajectory = Some(segment);
        self.trajectory_revision = self.trajectory_revision.wrapping_add(1);
        self.trajectory_pair_revision = self.trajectory_pair_revision.wrapping_add(1);
    }

    pub(crate) fn set_trajectory_time(
        &mut self,
        sample_seconds: f32,
    ) -> Result<(), crate::CoreError> {
        let trajectory = self
            .trajectory
            .as_mut()
            .ok_or(crate::CoreError::InvalidTrajectory {
                reason: "structure has no active trajectory segment",
            })?;
        let previous = trajectory.sample_seconds();
        trajectory.set_sample_time(sample_seconds)?;
        if trajectory.sample_seconds().to_bits() != previous.to_bits() {
            self.trajectory_revision = self.trajectory_revision.wrapping_add(1);
        }
        Ok(())
    }

    pub(crate) fn clear_trajectory(&mut self) -> bool {
        let changed = self.trajectory.take().is_some();
        self.trajectory_bvh = None;
        if changed {
            self.trajectory_revision = self.trajectory_revision.wrapping_add(1);
            self.trajectory_pair_revision = self.trajectory_pair_revision.wrapping_add(1);
        }
        changed
    }
}

fn atom_bvh(atoms: &AtomTable) -> Bvh {
    Bvh::build(&atom_aabbs(atoms))
}

fn atom_bounds(atoms: &AtomTable) -> Aabb {
    atoms
        .coords()
        .slice()
        .iter()
        .zip(atoms.radius().values())
        .fold(Aabb::EMPTY, |bounds, (center, radius)| {
            bounds.union(&atom_aabb(*center, *radius))
        })
}

fn atom_aabbs(atoms: &AtomTable) -> Vec<Aabb> {
    atoms
        .coords()
        .slice()
        .iter()
        .zip(atoms.radius().values())
        .map(|(center, radius)| atom_aabb(*center, *radius))
        .collect()
}

fn atom_aabb(center: [f32; 3], radius: f32) -> Aabb {
    let center = Vec3::from_array(center);
    let extent = Vec3::splat(radius.abs());
    Aabb::new(center - extent, center + extent)
}

fn trajectory_bvh(segment: &TrajectorySegment, radii: &[f32]) -> Bvh {
    let bounds = segment
        .start()
        .positions()
        .iter()
        .zip(segment.end().positions())
        .zip(radii)
        .map(|((start, end), radius)| {
            let start = Vec3::from_array(*start);
            let end = Vec3::from_array(*end);
            let extent = Vec3::splat(radius.abs());
            Aabb::new(start.min(end) - extent, start.max(end) + extent)
        })
        .collect::<Vec<_>>();
    Bvh::build(&bounds)
}
