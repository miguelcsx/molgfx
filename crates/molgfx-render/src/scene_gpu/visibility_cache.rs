//! Content-keyed sharing of the culling outputs.
//!
//! The cull shaders read the packed records, the frame, the placement transform
//! and the tiles. Of those, a representation can only vary the records it packs
//! and three scalar policies: the level-of-detail mode, whether a visual result
//! table participates, and the bond-break length. Everything else — colour,
//! opacity, material, visual style — is applied after culling and must not
//! appear here, which is what lets two representations with the same geometry
//! and different appearance share one visible set and one dispatch.

use super::buffers::CullCountInput;
use super::record_cache::RecordKey;
use crate::error::RenderError;
use molgfx_gpu::Device;

#[cfg(test)]
#[path = "visibility_cache_tests.rs"]
mod tests;

/// Everything the cull shaders read that a representation may vary.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct VisibilityKey {
    pub(crate) records: RecordKey,
    /// `CullCounts.lod_enabled`: 0, 1, or 2.
    pub(crate) lod_mode: u32,
    /// Whether a typed visual result table affects culling.
    pub(crate) visual_enabled: bool,
    /// `CullCounts.bond_break_length` bit pattern; zero disables breaking.
    pub(crate) bond_break_length: u32,
}

/// One shared visible set and its cull counts.
#[derive(Debug)]
pub(super) struct VisibilitySet<D: Device> {
    /// The counts last written, so an unchanged frame uploads nothing.
    written: Option<CullCountInput>,
    visible_atoms: Option<D::Buffer>,
    visible_bonds: Option<D::Buffer>,
    counts: Option<D::Buffer>,
    visible_atoms_capacity: u64,
    visible_bonds_capacity: u64,
    pub(super) atom_count: u32,
    pub(super) bond_count: u32,
}

impl<D: Device> Default for VisibilitySet<D> {
    fn default() -> Self {
        Self {
            written: None,
            visible_atoms: None,
            visible_bonds: None,
            counts: None,
            visible_atoms_capacity: 0,
            visible_bonds_capacity: 0,
            atom_count: 0,
            bond_count: 0,
        }
    }
}

/// Borrowed handle over one shared visible set.
#[derive(Debug)]
pub(super) struct VisibilitySetRef<'a, D: Device> {
    pub(super) visible_atoms: &'a D::Buffer,
    pub(super) visible_bonds: &'a D::Buffer,
    pub(super) counts: &'a D::Buffer,
}

impl<D: Device> Clone for VisibilitySetRef<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D: Device> Copy for VisibilitySetRef<'_, D> {}

/// Key-sorted cache of shared visible sets.
#[derive(Debug)]
pub(crate) struct VisibilityCache<D: Device> {
    entries: Vec<(VisibilityKey, VisibilitySet<D>)>,
}

impl<D: Device> VisibilityCache<D> {
    pub(super) const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Allocates the visible lists and counts for every key in `needed`.
    ///
    /// `counts` runs once per key, never once per representation.
    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        needed: &std::collections::BTreeMap<VisibilityKey, (u32, u32, CullCountInput)>,
    ) -> Result<(), RenderError> {
        for (key, (atom_count, bond_count, counts)) in needed {
            let position = match self.entries.binary_search_by_key(key, |(key, _)| *key) {
                Ok(position) => {
                    // Only a changed policy rewrites its counts; an unchanged
                    // frame must upload nothing but its own uniforms.
                    self.entries[position].1.sync_counts(queue, *counts);
                    position
                }
                Err(position) => {
                    let mut set = VisibilitySet::default();
                    set.sync(device, queue, *atom_count, *bond_count, *counts)?;
                    self.entries.insert(position, (*key, set));
                    position
                }
            };
            let _ = position;
        }
        self.entries.retain(|(key, _)| needed.contains_key(key));
        Ok(())
    }

    #[must_use]
    pub(super) fn get(&self, key: VisibilityKey) -> Option<&VisibilitySet<D>> {
        self.entries
            .binary_search_by_key(&key, |(key, _)| *key)
            .ok()
            .and_then(|index| self.entries.get(index))
            .map(|(_, set)| set)
    }

    /// The keys currently resident.
    pub(crate) fn keys(&self) -> impl Iterator<Item = VisibilityKey> + '_ {
        self.entries.iter().map(|(key, _)| *key)
    }
}

impl<D: Device> VisibilitySet<D> {
    fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        atom_count: u32,
        bond_count: u32,
        counts: CullCountInput,
    ) -> Result<(), RenderError> {
        self.atom_count = atom_count;
        self.bond_count = bond_count;
        super::buffers::ensure_indices(
            device,
            "visible atom indices",
            atom_count.saturating_mul(2),
            &mut self.visible_atoms,
            &mut self.visible_atoms_capacity,
        )?;
        super::buffers::ensure_indices(
            device,
            "visible bond indices",
            bond_count,
            &mut self.visible_bonds,
            &mut self.visible_bonds_capacity,
        )?;
        self.ensure_counts(device)?;
        self.sync_counts(queue, counts);
        Ok(())
    }

    fn ensure_counts(&mut self, device: &D) -> Result<(), RenderError> {
        if self.counts.is_none() {
            self.counts = Some(device.create_buffer(&molgfx_gpu::BufferDesc {
                label: "cull counts",
                size: std::mem::size_of::<super::slot_types::CullCounts>() as u64,
                usage: molgfx_gpu::BufferUsage::UNIFORM.union(molgfx_gpu::BufferUsage::COPY_DST),
            })?);
        }
        Ok(())
    }

    /// Writes the counts only when they differ from the resident ones.
    fn sync_counts(&mut self, queue: &D::Queue, input: CullCountInput) {
        if self.written == Some(input) {
            return;
        }
        let Some(counts) = self.counts.as_ref() else {
            return;
        };
        super::buffers::write_counts_into::<D>(queue, counts, input);
        self.written = Some(input);
    }

    #[must_use]
    pub(super) fn as_ref(&self) -> Option<VisibilitySetRef<'_, D>> {
        Some(VisibilitySetRef {
            visible_atoms: self.visible_atoms.as_ref()?,
            visible_bonds: self.visible_bonds.as_ref()?,
            counts: self.counts.as_ref()?,
        })
    }

    pub(crate) fn resident_bytes(&self) -> u64 {
        self.visible_atoms_capacity
            .saturating_add(self.visible_bonds_capacity)
            .saturating_add(std::mem::size_of::<super::slot_types::CullCounts>() as u64)
    }
}
