//! Content-keyed sharing of the packed instance records.
//!
//! Every input that forces a repack is in [`RecordKey`]; everything else a
//! representation reads is applied per draw. Two representations whose keys
//! match therefore want byte-identical records, and one upload serves both.
//!
//! Keeping the records here rather than in the slot is what makes
//! `GpuScene::sync`'s two passes natural: the slot list is walked read-only to
//! pack the frame's misses, then walked mutably to build bind groups against
//! the now-resident sets. A single pass cannot do both, because the cache and
//! the slots would be borrowed mutably at once.
//!
//! The packed CPU slices live only inside [`RecordCache::sync`]. They are
//! uploaded and consumed into the shared quality hierarchy in the same
//! iteration, so a resident record set costs device bytes and nothing else.

use super::acceleration_cache::AccelerationCache;
use crate::error::RenderError;
use crate::scene_gpu::asset::GpuAssetIdentity;
use crate::scene_gpu::buffers::{count, upload_grow};
use crate::scene_gpu::slot_types::{RecordState, SelectionIdentity, SelectionKey};
use molgfx_core::{AtomGpu, BondGpu, PlacedStructure, Representation, RepresentationKind, Scene};
use molgfx_gpu::Device;
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "record_cache_tests.rs"]
mod tests;

/// Everything that forces a repack, and nothing else.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct RecordKey {
    pub(crate) selection: SelectionKey,
    pub(crate) asset: GpuAssetIdentity,
    pub(crate) records: RecordState,
    /// Content revisions of the two physical columns the packer reads.
    pub(crate) properties: [u64; 2],
    /// Swept analytic bounds depend on the resident trajectory endpoint pair.
    pub(crate) trajectory: [u64; 2],
}

/// The projection of a record key that a *sampled geometry* depends on.
///
/// A surface field samples atom centres and radii, so it depends on which
/// records were packed and on the two scalar inputs that set a radius. It does
/// not depend on colour, appearance or surface pattern, which are applied when
/// the field is drawn. Projecting rather than reusing the whole key is what
/// lets two surfaces that differ only in appearance share one field.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct RecordGeometry {
    /// Which records were packed, and from which structure revision.
    pub(crate) selection: SelectionKey,
    /// The asset whose coordinates and topology those records came from.
    pub(crate) asset: GpuAssetIdentity,
    /// Content revisions of the physical columns the packer reads.
    pub(crate) properties: [u64; 2],
    /// The representation kind, which selects the packing rule.
    pub(crate) kind: u8,
    /// The radius scalar the packed radii were scaled by.
    pub(crate) radius_scale: u32,
}

impl RecordKey {
    /// The geometry a sampled field over these records depends on.
    #[must_use]
    pub(crate) const fn geometry(self) -> RecordGeometry {
        RecordGeometry {
            selection: self.selection,
            asset: self.asset,
            properties: self.properties,
            kind: self.records.kind(),
            radius_scale: self.records.radius_scale_bits(),
        }
    }
}

/// Borrowed inputs for one packing pass.
pub(in crate::scene_gpu) struct RecordPrepare<'a, D: Device> {
    pub(in crate::scene_gpu) device: &'a D,
    pub(in crate::scene_gpu) queue: &'a D::Queue,
    pub(in crate::scene_gpu) placed: &'a PlacedStructure,
    pub(in crate::scene_gpu) representation: &'a Representation,
    pub(in crate::scene_gpu) selection: &'a molgfx_core::AtomSelection,
    pub(in crate::scene_gpu) color: molgfx_geometry::ColorContext<'a>,
}

/// The ledger and frame a shared resource registers itself against.
pub(in crate::scene_gpu) struct RecordBudget<'a, D: Device> {
    pub(in crate::scene_gpu) ledger: &'a mut crate::DerivedCache,
    pub(in crate::scene_gpu) frame: u64,
    pub(in crate::scene_gpu) _device: std::marker::PhantomData<D>,
}

/// Caller-owned packing scratch, reused across every key in one frame.
pub(in crate::scene_gpu) struct RecordScratch<'a> {
    pub(in crate::scene_gpu) atoms: &'a mut Vec<AtomGpu>,
    pub(in crate::scene_gpu) bonds: &'a mut Vec<BondGpu>,
    pub(in crate::scene_gpu) compaction: &'a mut Vec<u32>,
}

/// One shared pair of packed record buffers.
#[derive(Debug)]
pub(in crate::scene_gpu) struct RecordSet<D: Device> {
    atoms: Option<D::Buffer>,
    bonds: Option<D::Buffer>,
    compaction: Option<D::Buffer>,
    pub(in crate::scene_gpu) atom_count: u32,
    pub(in crate::scene_gpu) bond_count: u32,
    atoms_capacity: u64,
    bonds_capacity: u64,
    compaction_capacity: u64,
}

impl<D: Device> Default for RecordSet<D> {
    fn default() -> Self {
        Self {
            atoms: None,
            bonds: None,
            compaction: None,
            atom_count: 0,
            bond_count: 0,
            atoms_capacity: 0,
            bonds_capacity: 0,
            compaction_capacity: 0,
        }
    }
}

/// Borrowed handle over one shared set, handed to a slot's sync.
#[derive(Debug)]
pub(in crate::scene_gpu) struct RecordSetRef<'a, D: Device> {
    pub(in crate::scene_gpu) atoms: &'a D::Buffer,
    pub(in crate::scene_gpu) bonds: &'a D::Buffer,
    pub(in crate::scene_gpu) compaction: &'a D::Buffer,
    pub(in crate::scene_gpu) atom_count: u32,
    pub(in crate::scene_gpu) bond_count: u32,
}

/// The derived `Copy` would require `D: Copy`, which no device is.
impl<D: Device> Clone for RecordSetRef<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D: Device> Copy for RecordSetRef<'_, D> {}

/// Key-sorted cache of shared record sets.
#[derive(Debug)]
pub(crate) struct RecordCache<D: Device> {
    entries: Vec<(RecordKey, RecordSet<D>)>,
}

impl<D: Device> RecordCache<D> {
    pub(in crate::scene_gpu) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// The key one representation packs under.
    #[must_use]
    pub(in crate::scene_gpu) fn key(
        scene: &Scene,
        placed: &PlacedStructure,
        structure: molgfx_core::StructureHandle,
        representation: &Representation,
        selection_handle: molgfx_core::SelectionHandle,
        asset: GpuAssetIdentity,
        property_revisions: [u64; 2],
    ) -> RecordKey {
        RecordKey {
            trajectory: if placed.trajectory().is_some() {
                [scene.cache_identity(), placed.trajectory_pair_revision()]
            } else {
                [0; 2]
            },
            selection: SelectionKey {
                structure,
                identity: match scene.selection_fingerprint(selection_handle) {
                    Some(fingerprint) => SelectionIdentity::Query(fingerprint),
                    None => SelectionIdentity::Mask {
                        scene: scene.cache_identity(),
                        handle: selection_handle,
                    },
                },
                topology: if representation.kind == RepresentationKind::DynamicBonds {
                    placed.trajectory_pair_revision()
                } else {
                    placed.bond_topology_revision()
                },
            },
            asset,
            records: RecordState::new(representation),
            // Only residue beads bake a colour into their records; every other
            // form resolves colour on the GPU, so a column edit must not
            // invalidate records that never read it.
            properties: if representation.kind == RepresentationKind::Beads {
                property_revisions
            } else {
                [0; 2]
            },
        }
    }

    /// Packs every key in `needed` that is not resident, releases every
    /// resident key outside it, and keeps the shared quality hierarchy in step.
    ///
    /// `prepare` runs once per key, never once per representation, so a frame
    /// with eight representations over one selection packs and builds once.
    pub(in crate::scene_gpu) fn sync<'a>(
        &mut self,
        acceleration: &mut AccelerationCache<D>,
        budget: &mut RecordBudget<'_, D>,
        needed: &BTreeMap<RecordKey, usize>,
        requires_bvh: bool,
        scratch: &mut RecordScratch<'_>,
        mut prepare: impl FnMut(usize, RecordKey) -> Result<Option<RecordPrepare<'a, D>>, RenderError>,
    ) -> Result<(), RenderError> {
        // A record set the ledger no longer holds is not resident, and is
        // rebuilt from the packer the next time a representation needs it.
        let (ledger, frame) = (&mut *budget.ledger, budget.frame);
        self.entries.retain(|(key, _)| needed.contains_key(key));
        let mut fresh = Vec::new();
        let mut hierarchy_inputs = Vec::new();
        for (key, slot_index) in needed {
            if self
                .entries
                .binary_search_by_key(key, |(key, _)| *key)
                .is_ok()
            {
                continue;
            }
            let Some(input) = prepare(*slot_index, *key)? else {
                continue;
            };
            let set = RecordSet::sync(&input, scratch)?;
            if requires_bvh {
                acceleration.build(
                    *key,
                    input.device,
                    input.queue,
                    scratch.atoms,
                    scratch.bonds,
                    input.placed,
                )?;
                hierarchy_inputs.push((*key, input.placed));
            }
            fresh.push((*key, set));
        }
        for (key, set) in fresh {
            self.admit(ledger, key, set, frame);
        }
        self.entries.retain(|(key, _)| needed.contains_key(key));
        acceleration.retain(needed, requires_bvh, ledger, frame);
        let _ = hierarchy_inputs;
        Ok(())
    }

    /// Makes a freshly packed set resident and registers it with the ledger.
    ///
    /// Record sets are the geometry every draw reads, not an optional
    /// derived cache. A set the budget refuses (one larger than the whole
    /// limit) therefore stays resident and unaccounted: dropping it would
    /// draw nothing and repack and re-upload every atom on every frame.
    pub(in crate::scene_gpu) fn admit(
        &mut self,
        ledger: &mut crate::DerivedCache,
        key: RecordKey,
        set: RecordSet<D>,
        frame: u64,
    ) {
        let _accounted = ledger.retain(
            key,
            crate::DerivedCacheClass::RecordSet,
            crate::DerivedFootprint {
                cpu_bytes: 0,
                gpu_bytes: set.resident_bytes(),
            },
            frame,
        );
        let position = match self.entries.binary_search_by_key(&key, |(key, _)| *key) {
            Ok(position) | Err(position) => position,
        };
        self.entries.insert(position, (key, set));
    }

    /// The shared set for one key, when resident.
    #[must_use]
    pub(in crate::scene_gpu) fn get(&self, key: RecordKey) -> Option<&RecordSet<D>> {
        self.entries
            .binary_search_by_key(&key, |(key, _)| *key)
            .ok()
            .and_then(|index| self.entries.get(index))
            .map(|(_, set)| set)
    }
}

impl<D: Device> RecordSet<D> {
    fn sync(
        input: &RecordPrepare<'_, D>,
        scratch: &mut RecordScratch<'_>,
    ) -> Result<Self, RenderError> {
        let mut set = Self::default();
        super::record_pack::pack_records(input, scratch)?;
        set.atom_count = count(scratch.atoms.len());
        set.bond_count = count(scratch.bonds.len());
        upload_grow(
            input.device,
            input.queue,
            "atom instances",
            scratch.atoms,
            &mut set.atoms,
            &mut set.atoms_capacity,
        )?;
        if input.representation.kind == molgfx_core::RepresentationKind::Beads {
            let mut recipes = Vec::new();
            crate::scene_gpu::bead_recipes::append_recipes(
                input.placed,
                input.selection,
                scratch.atoms,
                scratch.compaction,
                input.representation.params.radius_scale,
                &mut recipes,
            )?;
            upload_grow(
                input.device,
                input.queue,
                "residue coordinate recipes",
                &recipes,
                &mut set.bonds,
                &mut set.bonds_capacity,
            )?;
        } else {
            upload_grow(
                input.device,
                input.queue,
                "bond instances",
                scratch.bonds,
                &mut set.bonds,
                &mut set.bonds_capacity,
            )?;
        }
        upload_grow(
            input.device,
            input.queue,
            "source-to-compacted atom indices",
            scratch.compaction,
            &mut set.compaction,
            &mut set.compaction_capacity,
        )?;
        Ok(set)
    }

    /// Borrowed handle for a slot's bind-group construction.
    #[must_use]
    pub(in crate::scene_gpu) fn as_ref(&self) -> Option<RecordSetRef<'_, D>> {
        Some(RecordSetRef {
            atoms: self.atoms.as_ref()?,
            bonds: self.bonds.as_ref()?,
            compaction: self.compaction.as_ref()?,
            atom_count: self.atom_count,
            bond_count: self.bond_count,
        })
    }

    fn resident_bytes(&self) -> u64 {
        self.atoms_capacity
            .saturating_add(self.bonds_capacity)
            .saturating_add(self.compaction_capacity)
    }
}
