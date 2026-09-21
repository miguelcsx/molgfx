//! A structure placed in a scene.
//!
//! Placement pairs the immutable parsed structure with a model transform and
//! the scene's per-atom tables. Multiple placements of the same structure
//! share its storage; each carries its own transform and columns.

use crate::atoms::AtomTable;
use crate::hierarchy::Hierarchy;
use crate::{
    BondTopologySegment, Column, DatasetId, SecondaryStructure, StructureAsset, TrajectorySegment,
};
use molgfx_math::{Aabb, Bvh, BvhBuildScratch, BvhSource, Mat4, SweptSphereBounds};
use std::sync::Arc;

/// One structure in the scene, with its transform and derived tables.
#[derive(Clone, Debug)]
pub struct PlacedStructure {
    asset: StructureAsset,
    /// The parsed structure; a cheap reference-counted handle.
    pub structure: molframe::Structure,
    /// Model-to-world transform.
    pub model_to_world: Mat4,
    /// The dense per-atom table for the placed model.
    pub atoms: Arc<AtomTable>,
    /// Offset-array hierarchy over the structure's topology.
    pub hierarchy: Arc<Hierarchy>,
    /// Per-residue secondary structure supplied by the caller or `molframe`.
    pub secondary_structure: Column<SecondaryStructure>,
    trajectory: Option<TrajectorySegment>,
    trajectory_bvh: Option<Bvh>,
    trajectory_scratch: BvhBuildScratch,
    trajectory_revision: u64,
    trajectory_pair_revision: u64,
    bond_topology: Option<BondTopologySegment>,
    bond_topology_revision: u64,
    bond_topology_pair_revision: u64,
    bond_break_length: f32,
}

impl PlacedStructure {
    /// Places the first model of a structure at the identity transform.
    #[must_use]
    pub fn new(structure: &molframe::Structure) -> Option<Self> {
        let Ok(asset) = StructureAsset::new(DatasetId::LEGACY, structure) else {
            return None;
        };
        Some(Self::from_asset(&asset))
    }

    /// Places a validated shared asset at the identity transform.
    #[must_use]
    pub fn from_asset(asset: &StructureAsset) -> Self {
        let atoms = asset.shared_atoms();
        let hierarchy = asset.shared_hierarchy();
        let secondary_structure =
            Column::new(vec![SecondaryStructure::Unknown; hierarchy.residue_count()]);
        Self {
            asset: asset.clone(),
            structure: asset.structure().clone(),
            model_to_world: Mat4::IDENTITY,
            hierarchy,
            secondary_structure,
            atoms,
            trajectory: None,
            trajectory_bvh: None,
            trajectory_scratch: BvhBuildScratch::default(),
            trajectory_revision: 0,
            trajectory_pair_revision: 0,
            bond_topology: None,
            bond_topology_revision: 0,
            bond_topology_pair_revision: 0,
            bond_break_length: 0.0,
        }
    }

    /// Shared immutable source asset retained by this placement.
    #[must_use]
    pub const fn asset(&self) -> &StructureAsset {
        &self.asset
    }

    /// Caller-owned global dataset identity.
    #[must_use]
    pub fn dataset_id(&self) -> DatasetId {
        self.asset.dataset_id()
    }

    /// Maximum drawn length, in model units, before a bond is hidden as broken.
    ///
    /// Zero (the default) disables breaking, so a static structure draws every
    /// bond. A positive length lets the GPU cull a bond whose two endpoints have
    /// separated past a covalent cutoff — the real behaviour when a bond
    /// dissociates during a trajectory, instead of a cylinder stretching like
    /// taffy between atoms that are no longer bonded.
    #[must_use]
    pub const fn bond_break_length(&self) -> f32 {
        self.bond_break_length
    }

    pub(crate) fn set_bond_break_length(&mut self, length: f32) -> Result<(), crate::CoreError> {
        if !length.is_finite() || length < 0.0 {
            return Err(crate::CoreError::InvalidTrajectory {
                reason: "bond break length must be finite and non-negative",
            });
        }
        self.bond_break_length = length;
        Ok(())
    }

    /// World-space bound of the active coordinate interval, `O(1)`.
    #[must_use]
    pub fn world_aabb(&self) -> Aabb {
        self.trajectory_bvh
            .as_ref()
            .map_or(self.asset.spatial_bounds(), Bvh::bounds)
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

    /// Active caller-decoded dynamic covalent topology interval.
    #[must_use]
    pub const fn bond_topology(&self) -> Option<&BondTopologySegment> {
        self.bond_topology.as_ref()
    }

    /// Revision of the topology pair or its transition sample.
    #[must_use]
    pub const fn bond_topology_revision(&self) -> u64 {
        self.bond_topology_revision
    }

    /// Revision of the resident connectivity pair, excluding time-only updates.
    #[must_use]
    pub const fn bond_topology_pair_revision(&self) -> u64 {
        self.bond_topology_pair_revision
    }

    /// Hierarchy conservatively enclosing every active interpolated position.
    ///
    /// # Errors
    ///
    /// Returns the cached typed build error when the base hierarchy overflows.
    pub fn render_bvh(&self) -> Result<&Bvh, molgfx_math::BvhBuildError> {
        match &self.trajectory_bvh {
            Some(trajectory) => Ok(trajectory),
            None => self.spatial_bvh(),
        }
    }

    /// Lazily materializes the atom hierarchy only for spatial work.
    ///
    /// # Errors
    ///
    /// Returns the cached typed build error when compact GPU offsets overflow.
    pub fn spatial_bvh(&self) -> Result<&Bvh, molgfx_math::BvhBuildError> {
        self.asset.spatial_bvh()
    }

    #[cfg(test)]
    pub(crate) fn spatial_bvh_is_ready(&self) -> bool {
        self.asset.spatial_bvh_is_ready()
    }

    /// Replaces the active interval and brings its hierarchy up to date.
    ///
    /// Playback swaps coordinates without changing which atoms exist, so the
    /// existing topology still addresses the right primitives and only the
    /// bounds have moved. Refitting costs `O(primitives + nodes)` with no
    /// allocation, where a rebuild would re-key and re-sort every atom on every
    /// frame. Anything that changes the primitive count falls back to a
    /// rebuild, which reuses the retained sort scratch.
    pub(crate) fn replace_trajectory(
        &mut self,
        segment: TrajectorySegment,
    ) -> Result<(), crate::CoreError> {
        let source = SweptSphereBounds::new(
            segment.start().positions(),
            segment.end().positions(),
            self.atoms.radius().values(),
        );
        let mut hierarchy = match self.trajectory_bvh.take() {
            Some(hierarchy) => hierarchy,
            None => Bvh::default(),
        };
        // Equal counts mean every primitive survived the previous build, so the
        // permutation still names the same atoms.
        let refittable =
            !hierarchy.nodes.is_empty() && hierarchy.primitive_indices.len() == source.len();
        let outcome = if refittable {
            hierarchy.refit(&source)
        } else {
            hierarchy.rebuild(&source, &mut self.trajectory_scratch)
        };
        if outcome.is_err() {
            return Err(crate::CoreError::InvalidTrajectory {
                reason: "trajectory exceeds the compact GPU BVH index space",
            });
        }
        self.trajectory_bvh = Some(hierarchy);
        self.trajectory = Some(segment);
        self.trajectory_revision = self.trajectory_revision.wrapping_add(1);
        self.trajectory_pair_revision = self.trajectory_pair_revision.wrapping_add(1);
        Ok(())
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

    pub(crate) fn replace_bond_topology(&mut self, segment: BondTopologySegment) {
        self.bond_topology = Some(segment);
        self.bond_topology_revision = self.bond_topology_revision.wrapping_add(1);
        self.bond_topology_pair_revision = self.bond_topology_pair_revision.wrapping_add(1);
    }

    pub(crate) fn set_bond_topology_time(
        &mut self,
        sample_seconds: f32,
    ) -> Result<(), crate::CoreError> {
        let topology = self
            .bond_topology
            .as_mut()
            .ok_or(crate::CoreError::InvalidTrajectory {
                reason: "structure has no active dynamic topology segment",
            })?;
        let previous = topology.sample_seconds();
        topology.set_sample_time(sample_seconds)?;
        if topology.sample_seconds().to_bits() != previous.to_bits() {
            self.bond_topology_revision = self.bond_topology_revision.wrapping_add(1);
        }
        Ok(())
    }

    pub(crate) fn clear_bond_topology(&mut self) -> bool {
        let changed = self.bond_topology.take().is_some();
        if changed {
            self.bond_topology_revision = self.bond_topology_revision.wrapping_add(1);
            self.bond_topology_pair_revision = self.bond_topology_pair_revision.wrapping_add(1);
        }
        changed
    }
}
