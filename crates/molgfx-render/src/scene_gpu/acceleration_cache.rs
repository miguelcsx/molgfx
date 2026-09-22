//! Key-shared quality acceleration.
//!
//! The CPU BVH, its packed bond hierarchy and the optional hardware BLAS are
//! functions of the packed records, not of how a representation draws them, so
//! they are built once per [`RecordKey`] and shared. The placement TLAS stays
//! per slot because it carries the model transform.
//!
//! Building here rather than in the slot is also what lets the packed CPU
//! slices stay transient: they are consumed into the hierarchy during the same
//! pass that uploads them, so a resident record set costs device bytes only.

use super::quality_acceleration::QualityAcceleration;
use super::quality_hardware::QualityBlas;
use super::record_cache::RecordKey;
use crate::error::RenderError;
use molgfx_core::{AtomGpu, BondGpu, PlacedStructure};
use molgfx_gpu::Device;
use std::collections::BTreeMap;

/// Per-key quality acceleration, built once and drawn by every sharer.
#[derive(Debug)]
pub(super) struct AccelerationCache<D: Device> {
    entries: Vec<(RecordKey, SharedAcceleration<D>)>,
}

/// Everything derived from one record set that a placement instantiates.
#[derive(Debug)]
pub(super) struct SharedAcceleration<D: Device> {
    hierarchy: QualityAcceleration<D>,
    hardware: QualityBlas<D>,
}

impl<D: Device> SharedAcceleration<D> {
    pub(super) fn hierarchy(&self) -> &QualityAcceleration<D> {
        &self.hierarchy
    }

    /// Instantiates this geometry's hardware BLAS under one placement.
    pub(super) fn blas(&self) -> Option<&D::Blas> {
        self.hardware.blas()
    }

    pub(super) fn record(&mut self, encoder: &mut D::CommandEncoder) {
        self.hardware.record(encoder);
    }

    /// Resident device bytes of this shared geometry.
    #[must_use]
    pub(super) fn resident_bytes(&self) -> u64 {
        self.hierarchy
            .resident_bytes()
            .saturating_add(self.hardware.resident_bytes())
    }
}

impl<D: Device> AccelerationCache<D> {
    pub(super) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Builds the hierarchy for one freshly packed key.
    pub(super) fn build(
        &mut self,
        key: RecordKey,
        device: &D,
        queue: &D::Queue,
        atoms: &[AtomGpu],
        bonds: &[BondGpu],
        placed: &PlacedStructure,
    ) -> Result<(), RenderError> {
        let mut acceleration = SharedAcceleration {
            hierarchy: QualityAcceleration::new(),
            hardware: QualityBlas::new(),
        };
        acceleration
            .hierarchy
            .sync_topology(device, queue, atoms, bonds, placed)?;
        acceleration
            .hardware
            .sync(device, queue, atoms, bonds, placed);
        let position = match self.entries.binary_search_by_key(&key, |(key, _)| *key) {
            Ok(position) | Err(position) => position,
        };
        self.entries.insert(position, (key, acceleration));
        Ok(())
    }

    /// Releases every resident hierarchy outside `needed`, and everything when
    /// this frame does not require one.
    ///
    /// A hierarchy is only worth keeping while its record set is: the ledger
    /// evicts the two together, because the hierarchy cannot be rebuilt without
    /// the records it was built from.
    pub(super) fn retain(
        &mut self,
        needed: &BTreeMap<RecordKey, usize>,
        requires_bvh: bool,
        ledger: &mut crate::DerivedCache,
        frame: u64,
    ) {
        if !requires_bvh {
            self.entries.clear();
            return;
        }
        self.entries.retain(|(key, acceleration)| {
            ledger.retain(
                *key,
                crate::DerivedCacheClass::Acceleration,
                crate::DerivedFootprint {
                    cpu_bytes: 0,
                    gpu_bytes: acceleration.resident_bytes(),
                },
                frame,
            ) && needed.contains_key(key)
        });
    }

    #[must_use]
    pub(super) fn get(&self, key: RecordKey) -> Option<&SharedAcceleration<D>> {
        self.entries
            .binary_search_by_key(&key, |(key, _)| *key)
            .ok()
            .and_then(|index| self.entries.get(index))
            .map(|(_, acceleration)| acceleration)
    }

    /// Builds the hardware BLAS of every shared geometry, once per key.
    pub(super) fn record_hardware(&mut self, encoder: &mut D::CommandEncoder) {
        for (_, acceleration) in &mut self.entries {
            acceleration.record(encoder);
        }
    }
}
