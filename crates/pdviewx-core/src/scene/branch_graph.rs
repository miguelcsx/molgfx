//! Metadata-only branching over caller-decoded trajectory segments.

use crate::{CoreError, Scene, StructureHandle, TrajectorySegment};

#[cfg(test)]
#[path = "branch_graph_tests.rs"]
mod tests;

/// One named directed transition between caller-defined molecular states.
#[derive(Clone, PartialEq, Debug)]
pub struct TrajectoryBranch {
    from: u32,
    to: u32,
    event: Box<str>,
    probability: f32,
}

impl TrajectoryBranch {
    /// Creates one branch with optional Markov-state probability metadata.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for an empty event or a
    /// probability outside `[0, 1]`.
    pub fn new(
        from: u32,
        to: u32,
        event: impl Into<Box<str>>,
        probability: f32,
    ) -> Result<Self, CoreError> {
        let event = event.into();
        if event.trim().is_empty()
            || !probability.is_finite()
            || !(0.0..=1.0).contains(&probability)
        {
            return Err(invalid("trajectory branch metadata is invalid"));
        }
        Ok(Self {
            from,
            to,
            event,
            probability,
        })
    }

    /// Source state identifier.
    #[must_use]
    pub const fn from(&self) -> u32 {
        self.from
    }

    /// Target state identifier.
    #[must_use]
    pub const fn to(&self) -> u32 {
        self.to
    }

    /// Caller-defined event name.
    #[must_use]
    pub fn event(&self) -> &str {
        &self.event
    }

    /// Caller-supplied transition probability metadata.
    #[must_use]
    pub const fn probability(&self) -> f32 {
        self.probability
    }
}

/// Compact state graph that never retains decoded trajectory frames.
#[derive(Clone, Debug)]
pub struct TrajectoryStateGraph {
    states: Box<[u32]>,
    branches: Box<[TrajectoryBranch]>,
    active: u32,
}

impl TrajectoryStateGraph {
    /// Validates and indexes state/branch metadata once.
    ///
    /// The caller remains responsible for decoding the two-frame segment for a
    /// selected target. This keeps graph memory `O(states + branches)` instead
    /// of `O(all trajectory frames)`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for duplicate/missing states,
    /// duplicate `(source, event)` routes or an absent initial state.
    pub fn new(
        mut states: Vec<u32>,
        mut branches: Vec<TrajectoryBranch>,
        initial: u32,
    ) -> Result<Self, CoreError> {
        states.sort_unstable();
        if states.is_empty()
            || states.windows(2).any(|rows| rows[0] == rows[1])
            || states.binary_search(&initial).is_err()
            || branches.iter().any(|branch| {
                states.binary_search(&branch.from).is_err()
                    || states.binary_search(&branch.to).is_err()
            })
        {
            return Err(invalid("trajectory state graph references invalid states"));
        }
        branches.sort_unstable_by(|left, right| {
            (left.from, left.event.as_ref()).cmp(&(right.from, right.event.as_ref()))
        });
        if branches.windows(2).any(|rows| {
            rows[0].from == rows[1].from && rows[0].event.as_ref() == rows[1].event.as_ref()
        }) {
            return Err(invalid("trajectory state graph has an ambiguous event"));
        }
        Ok(Self {
            states: states.into_boxed_slice(),
            branches: branches.into_boxed_slice(),
            active: initial,
        })
    }

    /// Resolves the active state's target for an event in `O(log branches)`.
    #[must_use]
    pub fn target(&self, event: &str) -> Option<u32> {
        self.branches
            .binary_search_by(|branch| {
                (branch.from, branch.event.as_ref()).cmp(&(self.active, event))
            })
            .ok()
            .and_then(|row| self.branches.get(row))
            .map(TrajectoryBranch::to)
    }

    /// Applies a caller-decoded segment for one allowed event atomically.
    ///
    /// The active graph node changes only after the scene accepts the segment.
    /// No coordinate array is copied by the graph.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an unknown event, stale structure or segment
    /// topology mismatch.
    pub fn transition(
        &mut self,
        scene: &mut Scene,
        structure: StructureHandle,
        event: &str,
        segment: TrajectorySegment,
    ) -> Result<u32, CoreError> {
        let target = self
            .target(event)
            .ok_or_else(|| invalid("event is not reachable from the active trajectory state"))?;
        scene.set_trajectory_segment(structure, segment)?;
        self.active = target;
        Ok(target)
    }

    /// Seeks an existing state with a caller-decoded resident segment.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an absent state or invalid segment.
    pub fn seek(
        &mut self,
        scene: &mut Scene,
        structure: StructureHandle,
        state: u32,
        segment: TrajectorySegment,
    ) -> Result<(), CoreError> {
        if self.states.binary_search(&state).is_err() {
            return Err(invalid(
                "trajectory state graph does not contain the target",
            ));
        }
        scene.set_trajectory_segment(structure, segment)?;
        self.active = state;
        Ok(())
    }

    /// Active state identifier.
    #[must_use]
    pub const fn active(&self) -> u32 {
        self.active
    }

    /// Sorted state identifiers.
    #[must_use]
    pub fn states(&self) -> &[u32] {
        &self.states
    }

    /// Sorted transition metadata.
    #[must_use]
    pub fn branches(&self) -> &[TrajectoryBranch] {
        &self.branches
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidTimeline { reason }
}
