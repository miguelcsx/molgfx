//! Bounded caller-decoded dynamic covalent topology.
//!
//! The renderer retains two sorted connectivity rows and one merged transition
//! row. Building an interval is `O(start + end)`; sampling is `O(union)` and
//! reuses the merged allocation. Parsing and bond inference remain upstream.

use crate::{CoreError, EntityId};
use std::sync::Arc;

#[cfg(test)]
#[path = "topology_tests.rs"]
mod tests;

/// One canonical covalent connection in a decoded topology frame.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct TopologyBond {
    atoms: [u32; 2],
    aromatic: bool,
}

impl TopologyBond {
    /// Creates one connection with endpoints stored in ascending order.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a self-bond.
    pub fn new(atom_a: u32, atom_b: u32, aromatic: bool) -> Result<Self, CoreError> {
        if atom_a == atom_b {
            return Err(invalid("dynamic topology cannot contain self-bonds"));
        }
        Ok(Self {
            atoms: [atom_a.min(atom_b), atom_a.max(atom_b)],
            aromatic,
        })
    }

    /// Canonical endpoint indices into the structure atom table.
    #[must_use]
    pub const fn atoms(self) -> [u32; 2] {
        self.atoms
    }

    /// Whether this connection has aromatic presentation.
    #[must_use]
    pub const fn is_aromatic(self) -> bool {
        self.aromatic
    }

    const fn endpoint_key(self) -> [u32; 2] {
        self.atoms
    }
}

/// One decoded connectivity frame shared with the caller.
#[derive(Clone, PartialEq, Debug)]
pub struct BondTopologyFrame {
    index: u64,
    time_seconds: f32,
    atom_count: u32,
    bonds: Arc<[TopologyBond]>,
    provenance: Arc<str>,
}

impl BondTopologyFrame {
    /// Validates one canonical, endpoint-sorted topology row without copying it.
    ///
    /// Sorted input makes duplicate validation linear and lets two frames merge
    /// without a hash table or retained auxiliary index.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed time, atom count, endpoints, order,
    /// duplicates, excessive rows or empty provenance.
    pub fn new(
        index: u64,
        time_seconds: f32,
        atom_count: u32,
        bonds: Arc<[TopologyBond]>,
        provenance: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        if !time_seconds.is_finite() || atom_count == 0 {
            return Err(invalid("topology frame time and atom count must be valid"));
        }
        if bonds.len() > EntityId::MAX_INDEX as usize {
            return Err(invalid("topology frame exceeds the pickable row limit"));
        }
        if bonds
            .iter()
            .any(|bond| bond.atoms[1] >= atom_count || bond.atoms[0] >= bond.atoms[1])
        {
            return Err(invalid("topology bond endpoints escape the atom table"));
        }
        if bonds
            .windows(2)
            .any(|pair| pair[0].endpoint_key() >= pair[1].endpoint_key())
        {
            return Err(invalid(
                "topology bonds must be strictly endpoint-sorted without duplicates",
            ));
        }
        let provenance = provenance.into();
        if provenance.trim().is_empty() {
            return Err(invalid("topology frame provenance must not be empty"));
        }
        Ok(Self {
            index,
            time_seconds,
            atom_count,
            bonds,
            provenance,
        })
    }

    /// Stable caller frame index.
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }

    /// Physical or logical frame time.
    #[must_use]
    pub const fn time_seconds(&self) -> f32 {
        self.time_seconds
    }

    /// Fixed topology atom count.
    #[must_use]
    pub const fn atom_count(&self) -> u32 {
        self.atom_count
    }

    /// Sorted shared connectivity row.
    #[must_use]
    pub fn bonds(&self) -> &[TopologyBond] {
        &self.bonds
    }

    /// Source decoder, dataset or simulation identifier.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }
}

/// One bond in the resident union, with its current birth/death weight.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ActiveTopologyBond {
    bond: TopologyBond,
    weight: f32,
}

impl ActiveTopologyBond {
    /// Current connection semantics.
    #[must_use]
    pub const fn bond(self) -> TopologyBond {
        self.bond
    }

    /// Radius multiplier in `[0, 1]` for smooth formation or fracture.
    #[must_use]
    pub const fn weight(self) -> f32 {
        self.weight
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct TransitionBond {
    start: Option<TopologyBond>,
    end: Option<TopologyBond>,
    active: ActiveTopologyBond,
}

/// Two resident connectivity frames and their allocation-stable merged row.
#[derive(Clone, PartialEq, Debug)]
pub struct BondTopologySegment {
    start: BondTopologyFrame,
    end: BondTopologyFrame,
    sample_seconds: f32,
    interpolation: f32,
    merged: Vec<TransitionBond>,
}

impl BondTopologySegment {
    /// Builds one ordered dynamic-topology interval in linear time.
    ///
    /// # Errors
    ///
    /// Returns a typed error for mismatched topology, unordered frames or a
    /// sample outside the closed resident interval.
    pub fn new(
        start: BondTopologyFrame,
        end: BondTopologyFrame,
        sample_seconds: f32,
    ) -> Result<Self, CoreError> {
        if start.atom_count != end.atom_count {
            return Err(invalid("topology frames must use one fixed atom table"));
        }
        if start.index >= end.index || start.time_seconds >= end.time_seconds {
            return Err(invalid("topology frame indices and times must increase"));
        }
        let merged = merge_frames(start.bonds(), end.bonds())?;
        let mut segment = Self {
            start,
            end,
            sample_seconds: 0.0,
            interpolation: 0.0,
            merged,
        };
        segment.set_sample_time(sample_seconds)?;
        Ok(segment)
    }

    /// Updates transition weights without allocating.
    ///
    /// # Errors
    ///
    /// Returns a typed error outside the resident interval.
    pub fn set_sample_time(&mut self, sample_seconds: f32) -> Result<(), CoreError> {
        if !sample_seconds.is_finite()
            || sample_seconds < self.start.time_seconds
            || sample_seconds > self.end.time_seconds
        {
            return Err(invalid(
                "topology sample must lie within its resident interval",
            ));
        }
        let duration = self.end.time_seconds - self.start.time_seconds;
        let alpha = (sample_seconds - self.start.time_seconds) / duration;
        for transition in &mut self.merged {
            let weight = match (transition.start, transition.end) {
                (Some(_), Some(_)) => 1.0,
                (Some(_), None) => 1.0 - alpha,
                (None, Some(_)) => alpha,
                (None, None) => 0.0,
            };
            let bond = match (transition.start, transition.end) {
                (Some(start), Some(end)) if alpha >= 0.5 => end,
                (Some(start), _) => start,
                (None, Some(end)) => end,
                (None, None) => transition.active.bond,
            };
            transition.active = ActiveTopologyBond { bond, weight };
        }
        self.sample_seconds = sample_seconds;
        self.interpolation = alpha;
        Ok(())
    }

    /// Earlier resident frame.
    #[must_use]
    pub const fn start(&self) -> &BondTopologyFrame {
        &self.start
    }

    /// Later resident frame.
    #[must_use]
    pub const fn end(&self) -> &BondTopologyFrame {
        &self.end
    }

    /// Current sample time.
    #[must_use]
    pub const fn sample_seconds(&self) -> f32 {
        self.sample_seconds
    }

    /// Current interpolation fraction.
    #[must_use]
    pub const fn interpolation(&self) -> f32 {
        self.interpolation
    }

    /// Stable merged row. Zero-weight endpoint rows are retained for identity.
    #[must_use]
    pub fn bonds(&self) -> impl ExactSizeIterator<Item = &ActiveTopologyBond> {
        self.merged.iter().map(|transition| &transition.active)
    }

    /// Resolves one merged row in constant time.
    #[must_use]
    pub fn bond(&self, row: u32) -> Option<&ActiveTopologyBond> {
        self.merged.get(row as usize).map(|value| &value.active)
    }
}

fn merge_frames(
    start: &[TopologyBond],
    end: &[TopologyBond],
) -> Result<Vec<TransitionBond>, CoreError> {
    let capacity = start
        .len()
        .checked_add(end.len())
        .ok_or_else(|| invalid("topology union size overflowed"))?;
    if capacity > EntityId::MAX_INDEX as usize {
        return Err(invalid("topology union exceeds the pickable row limit"));
    }
    let mut merged = Vec::with_capacity(capacity);
    let (mut left, mut right) = (0, 0);
    while left < start.len() || right < end.len() {
        let a = start.get(left).copied();
        let b = end.get(right).copied();
        let (from, to) = match (a, b) {
            (Some(a), Some(b)) if a.endpoint_key() == b.endpoint_key() => {
                left += 1;
                right += 1;
                (Some(a), Some(b))
            }
            (Some(a), Some(b)) if a.endpoint_key() < b.endpoint_key() => {
                left += 1;
                (Some(a), None)
            }
            (Some(_) | None, Some(b)) => {
                right += 1;
                (None, Some(b))
            }
            (Some(a), None) => {
                left += 1;
                (Some(a), None)
            }
            (None, None) => break,
        };
        let bond = from
            .or(to)
            .ok_or_else(|| invalid("empty topology transition"))?;
        merged.push(TransitionBond {
            start: from,
            end: to,
            active: ActiveTopologyBond { bond, weight: 0.0 },
        });
    }
    Ok(merged)
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidTrajectory { reason }
}
