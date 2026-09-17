//! Atomic replacement of bounded placement and timeline declarations.

use super::ChunkGpuResidency;
use crate::engine::chunk_residency_support::{
    validate_instance_placements, validate_placements, validate_point_placements,
    validate_relation_placements,
};
use crate::engine::{
    AttributeChunkWindow, BondChunkPlacement, ChunkResidencyError, InstanceChunkPlacement,
    InstanceChunkWindow, PointChunkPlacement, RelationChunkPlacement, StructureChunkPlacement,
    TrajectoryChunkWindow,
};
use molgfx_gpu::Device;

impl<D: Device> ChunkGpuResidency<D> {
    pub(in crate::engine) fn replace_placements(
        &mut self,
        placements: &[StructureChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        validate_placements(placements, self.placements.capacity())?;
        if self.placements == placements {
            return Ok(());
        }
        self.placements.clear();
        self.placements.extend_from_slice(placements);
        self.placement_index.clear();
        self.placement_index.extend(
            self.placements
                .iter()
                .enumerate()
                .map(|(index, placement)| (placement.id, index)),
        );
        self.bump_revision();
        Ok(())
    }

    pub(in crate::engine) fn replace_bond_placements(
        &mut self,
        placements: &[BondChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        self.bonds.replace_placements(placements)?;
        Ok(())
    }

    pub(in crate::engine) fn replace_point_placements(
        &mut self,
        placements: &[PointChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        validate_point_placements(placements, self.point_placements.capacity())?;
        if self.point_placements == placements {
            return Ok(());
        }
        self.point_placements.clear();
        self.point_placements.extend_from_slice(placements);
        self.point_placement_index.clear();
        self.point_placement_index.extend(
            self.point_placements
                .iter()
                .enumerate()
                .map(|(index, placement)| (placement.id(), index)),
        );
        self.bump_revision();
        Ok(())
    }

    pub(in crate::engine) fn replace_instance_placements(
        &mut self,
        placements: &[InstanceChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        validate_instance_placements(placements, self.instance_placements.capacity())?;
        if self.instance_placements == placements {
            return Ok(());
        }
        self.instance_placements.clear();
        self.instance_placements.extend_from_slice(placements);
        self.instance_placement_index.clear();
        self.instance_placement_index.extend(
            self.instance_placements
                .iter()
                .enumerate()
                .map(|(index, placement)| (placement.id(), index)),
        );
        self.bump_revision();
        Ok(())
    }

    pub(in crate::engine) fn replace_relation_placements(
        &mut self,
        placements: &[RelationChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        validate_relation_placements(placements, self.relation_placements.capacity())?;
        if self.relation_placements == placements {
            return Ok(());
        }
        self.relation_placements.clear();
        self.relation_placements.extend_from_slice(placements);
        self.relation_placement_index.clear();
        self.relation_placement_index.extend(
            self.relation_placements
                .iter()
                .enumerate()
                .map(|(index, placement)| (placement.id(), index)),
        );
        self.bump_revision();
        Ok(())
    }

    pub(in crate::engine) fn replace_trajectory_windows(
        &mut self,
        windows: &[TrajectoryChunkWindow],
    ) -> Result<(), ChunkResidencyError> {
        if windows.len() > self.trajectory_windows.capacity() {
            return Err(ChunkResidencyError::TrackingCapacity);
        }
        let mut structures = hashbrown::HashSet::with_capacity(windows.len());
        for window in windows {
            if !structures.insert((window.structure.key, window.structure.generation())) {
                return Err(ChunkResidencyError::TrajectoryTopologyMismatch);
            }
        }
        if self.trajectory_windows != windows {
            self.trajectory_windows.clear();
            self.trajectory_windows.extend_from_slice(windows);
            self.bump_timeline_revision();
        }
        Ok(())
    }

    pub(in crate::engine) fn replace_instance_windows(
        &mut self,
        windows: &[InstanceChunkWindow],
    ) -> Result<(), ChunkResidencyError> {
        if windows.len() > self.instance_windows.capacity() {
            return Err(ChunkResidencyError::TrackingCapacity);
        }
        let mut next = windows.to_vec();
        next.sort_unstable_by_key(|window| window.placement);
        if next
            .windows(2)
            .any(|pair| pair[0].placement == pair[1].placement)
        {
            return Err(ChunkResidencyError::InstanceTimelineMismatch);
        }
        if self.instance_windows != next {
            let sources_changed = self.instance_windows.len() != next.len()
                || self
                    .instance_windows
                    .iter()
                    .zip(&next)
                    .any(|(left, right)| {
                        left.placement != right.placement
                            || left.start != right.start
                            || left.end != right.end
                    });
            self.instance_windows = next;
            self.bump_timeline_revision();
            self.instance_timeline_revision =
                self.instance_timeline_revision.wrapping_add(1).max(1);
            if sources_changed {
                self.relation_revision = self.relation_revision.wrapping_add(1).max(1);
            }
        }
        Ok(())
    }

    pub(in crate::engine) fn replace_attribute_windows(
        &mut self,
        windows: &[AttributeChunkWindow],
    ) -> Result<(), ChunkResidencyError> {
        if windows.len() > self.attribute_windows.capacity() {
            return Err(ChunkResidencyError::TrackingCapacity);
        }
        let mut next = windows.to_vec();
        next.sort_unstable_by_key(|window| window.attribute);
        if next
            .windows(2)
            .any(|pair| pair[0].attribute == pair[1].attribute)
        {
            return Err(ChunkResidencyError::AttributeTimelineMismatch);
        }
        if self.attribute_windows != next {
            self.attribute_windows = next;
            self.attribute_timeline_revision =
                self.attribute_timeline_revision.wrapping_add(1).max(1);
        }
        Ok(())
    }
}
