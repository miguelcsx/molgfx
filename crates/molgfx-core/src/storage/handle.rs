//! Generational handles and the slot map behind them.
//!
//! A handle stays valid across unrelated edits and detects its own staleness:
//! removing an entry bumps the slot's generation, so a handle held from
//! before the removal no longer resolves. All operations are `O(1)`.

#[cfg(test)]
#[path = "handle_tests.rs"]
mod tests;

use serde::{Deserialize, Serialize};

/// Identifies a placed structure within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct StructureHandle(pub(crate) RawHandle);

/// Identifies a representation within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RepresentationHandle(pub(crate) RawHandle);

/// Identifies a stored selection within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SelectionHandle(pub(crate) RawHandle);

/// Identifies a caller-supplied scalar volume within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct VolumeHandle(pub(crate) RawHandle);

/// Identifies a caller-supplied categorical label volume within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SegmentationHandle(pub(crate) RawHandle);

/// Identifies a caller-supplied interaction edge within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct InteractionHandle(pub(crate) RawHandle);

/// Identifies a caller-authored guide within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct GuideHandle(pub(crate) RawHandle);

/// Identifies a caller-supplied mesh within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MeshHandle(pub(crate) RawHandle);

/// Identifies one transform-only occurrence of a shared mesh.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct MeshInstanceHandle(pub(crate) RawHandle);

/// Identifies one depth-independent screen overlay.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct OverlayHandle(pub(crate) RawHandle);

/// Identifies a persistent annotation within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct AnnotationHandle(pub(crate) RawHandle);

/// Identifies a persistent measurement within a scene.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct MeasurementHandle(pub(crate) RawHandle);

/// Identifies one immutable caller-supplied typed attribute column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct AttributeHandle(pub(crate) RawHandle);

/// Transitional handle for the pre-schema-8 scalar-property table.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct AtomPropertyHandle(pub(crate) RawHandle);

/// Identifies one caller-supplied analytic primitive.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PrimitiveHandle(pub(crate) RawHandle);

/// Identifies one shared-layout point batch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PointBatchHandle(pub(crate) RawHandle);

/// Identifies one compact batch of rigid instances sharing an analytic template.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct InstanceBatchHandle(pub(crate) RawHandle);

/// Identifies one caller-supplied batch of generic spatial relations.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RelationBatchHandle(pub(crate) RawHandle);

/// Transitional handle for the pre-schema-8 pose table.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct LigandPoseBatchHandle(pub(crate) RawHandle);

/// Identifies one independently warped timeline track.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct TimelineTrackHandle(pub(crate) RawHandle);

macro_rules! handle_identity {
    ($($handle:ident),+ $(,)?) => {
        $(
            impl $handle {
                /// Stable slot index used by scene manifests and GPU tables.
                pub const fn row(self) -> u32 {
                    self.0.index
                }

                /// Generation component used to reject stale links.
                pub const fn generation(self) -> u32 {
                    self.0.generation
                }
            }
        )+
    };
}

handle_identity!(
    StructureHandle,
    RepresentationHandle,
    SelectionHandle,
    VolumeHandle,
    SegmentationHandle,
    AttributeHandle,
    AtomPropertyHandle,
    PrimitiveHandle,
    PointBatchHandle,
    InstanceBatchHandle,
    RelationBatchHandle,
    LigandPoseBatchHandle,
    TimelineTrackHandle,
    MeshHandle,
    MeshInstanceHandle,
    OverlayHandle,
);

impl InteractionHandle {
    pub(crate) const fn row(self) -> u32 {
        self.0.row()
    }
}

impl GuideHandle {
    pub(crate) const fn row(self) -> u32 {
        self.0.row()
    }
}

impl AnnotationHandle {
    pub(crate) const fn row(self) -> u32 {
        self.0.row()
    }
}

impl MeasurementHandle {
    pub(crate) const fn row(self) -> u32 {
        self.0.row()
    }
}

/// Slot index plus generation; the unit every typed handle wraps.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct RawHandle {
    index: u32,
    generation: u32,
}

impl RawHandle {
    pub(crate) const fn from_parts(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub(crate) const fn row(self) -> u32 {
        self.index
    }

    pub(crate) const fn generation(self) -> u32 {
        self.generation
    }
}

#[cfg(test)]
impl RawHandle {
    pub(crate) const fn new_for_test(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
}

/// A dense store with stable, generation-checked handles.
#[derive(Clone, Debug)]
pub(crate) struct SlotMap<T> {
    slots: Vec<Slot<T>>,
    sparse: Vec<SparseSlot<T>>,
    free: Vec<u32>,
    live: usize,
}

impl<T> Default for SlotMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

#[derive(Clone, Debug)]
struct SparseSlot<T> {
    index: u32,
    slot: Slot<T>,
}

/// Why a serialized row set cannot be represented safely by a slot map.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ManifestRowsError {
    /// Rows are duplicated or do not follow canonical ascending order.
    InconsistentOrder,
    /// A row cannot be represented without colliding with a sentinel.
    RowOutOfRange,
    /// The bounded backing storage could not be reserved.
    CapacityUnavailable,
}

impl ManifestRowsError {
    /// Stable caller-facing reason used by typed manifest errors.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::InconsistentOrder => "manifest rows are not strictly increasing",
            Self::RowOutOfRange => "manifest row is outside the supported identity range",
            Self::CapacityUnavailable => "manifest slot capacity is unavailable",
        }
    }
}

impl<T> SlotMap<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            sparse: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    /// Reserves backing slots for an insertion batch without changing handles.
    pub fn reserve(&mut self, additional: usize) {
        self.slots.reserve(additional);
    }

    /// Validates and reserves a canonical manifest row set before insertion.
    ///
    /// Rows above the dense prefix use one sparse record rather than allocating
    /// every intervening hole. The all-ones row is reserved so no manifest can
    /// collide with identity sentinels used by downstream GPU tables.
    pub fn prepare_manifest_rows<I>(&mut self, rows: I) -> Result<(), ManifestRowsError>
    where
        I: ExactSizeIterator<Item = u32>,
    {
        let live_count = rows.len();
        if live_count == 0 {
            return Ok(());
        }
        let mut previous = None;
        let mut dense_count = 0usize;
        let mut next_dense = crate::column::saturating_u32(self.slots.len());
        for row in rows {
            if row == u32::MAX {
                return Err(ManifestRowsError::RowOutOfRange);
            }
            if previous.is_some_and(|before| row <= before) {
                return Err(ManifestRowsError::InconsistentOrder);
            }
            if row == next_dense {
                dense_count = dense_count.saturating_add(1);
                next_dense = next_dense.saturating_add(1);
            }
            previous = Some(row);
        }
        self.slots
            .try_reserve_exact(dense_count)
            .map_err(|_| ManifestRowsError::CapacityUnavailable)?;
        self.sparse
            .try_reserve_exact(live_count.saturating_sub(dense_count))
            .map_err(|_| ManifestRowsError::CapacityUnavailable)
    }

    /// Inserts a value, reusing a freed slot when one exists.
    pub fn insert(&mut self, value: T) -> RawHandle {
        if let Some(index) = self.free.pop()
            && let Some(slot) = self.slot_mut(index)
            && slot.value.is_none()
        {
            let generation = slot.generation;
            slot.value = Some(value);
            self.live = self.live.saturating_add(1);
            return RawHandle { index, generation };
        }
        self.densify_sparse_prefix();
        let index = crate::column::saturating_u32(self.slots.len());
        self.slots.push(Slot {
            generation: 0,
            value: Some(value),
        });
        self.live = self.live.saturating_add(1);
        RawHandle {
            index,
            generation: 0,
        }
    }

    /// Inserts a value at a previously serialized identity.
    ///
    /// This is intentionally separate from [`Self::insert`]: normal scene
    /// edits allocate the next available identity, while rehydration must
    /// preserve rows and generations from a validated manifest. Large gaps are
    /// represented sparsely and therefore cost `O(live rows)`, not `O(max row)`.
    pub fn insert_at(&mut self, handle: RawHandle, value: T) -> Option<()> {
        if handle.index == u32::MAX {
            return None;
        }
        let index = usize::try_from(handle.index).ok()?;
        if self.slots.len() == index {
            self.slots.push(Slot {
                generation: handle.generation,
                value: Some(value),
            });
            self.live = self.live.saturating_add(1);
            self.densify_sparse_prefix();
            return Some(());
        }
        if index < self.slots.len() {
            let slot = self.slots.get_mut(index)?;
            if slot.value.is_some() {
                return None;
            }
            slot.generation = handle.generation;
            slot.value = Some(value);
            self.remove_free(handle.index)?;
            self.live = self.live.saturating_add(1);
            return Some(());
        }
        let position = match self
            .sparse
            .binary_search_by_key(&handle.index, |entry| entry.index)
        {
            Ok(position) => {
                let entry = self.sparse.get_mut(position)?;
                if entry.slot.value.is_some() {
                    return None;
                }
                entry.slot.generation = handle.generation;
                entry.slot.value = Some(value);
                self.remove_free(handle.index)?;
                self.live = self.live.saturating_add(1);
                return Some(());
            }
            Err(position) => position,
        };
        self.sparse.insert(
            position,
            SparseSlot {
                index: handle.index,
                slot: Slot {
                    generation: handle.generation,
                    value: Some(value),
                },
            },
        );
        self.live = self.live.saturating_add(1);
        Some(())
    }

    /// Resolves a handle, or `None` when it is stale.
    pub fn get(&self, handle: RawHandle) -> Option<&T> {
        let slot = self.slot(handle.index)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.value.as_ref()
    }

    /// Resolves a live row and returns its current generation-checked handle.
    pub fn get_index(&self, index: u32) -> Option<(RawHandle, &T)> {
        let slot = self.slot(index)?;
        Some((
            RawHandle {
                index,
                generation: slot.generation,
            },
            slot.value.as_ref()?,
        ))
    }

    /// Mutable resolution, or `None` when the handle is stale.
    pub fn get_mut(&mut self, handle: RawHandle) -> Option<&mut T> {
        let slot = self.slot_mut(handle.index)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.value.as_mut()
    }

    /// Removes an entry, invalidating every handle to it.
    pub fn remove(&mut self, handle: RawHandle) -> Option<T> {
        let slot = self.slot_mut(handle.index)?;
        if slot.generation != handle.generation {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(handle.index);
        self.live = self.live.saturating_sub(1);
        Some(value)
    }

    /// Iterates live entries in slot order, which is stable across removals
    /// of other entries.
    pub fn iter(&self) -> impl Iterator<Item = (RawHandle, &T)> + '_ {
        let dense = self.slots.iter().enumerate().filter_map(|(i, slot)| {
            let value = slot.value.as_ref()?;
            let index = crate::column::saturating_u32(i);
            Some((
                RawHandle {
                    index,
                    generation: slot.generation,
                },
                value,
            ))
        });
        let sparse = self.sparse.iter().filter_map(|entry| {
            let value = entry.slot.value.as_ref()?;
            Some((
                RawHandle {
                    index: entry.index,
                    generation: entry.slot.generation,
                },
                value,
            ))
        });
        dense.chain(sparse)
    }

    /// Number of live entries.
    pub fn len(&self) -> usize {
        self.live
    }

    fn slot(&self, index: u32) -> Option<&Slot<T>> {
        if let Some(slot) = self.slots.get(index as usize) {
            return Some(slot);
        }
        let position = self
            .sparse
            .binary_search_by_key(&index, |entry| entry.index)
            .ok()?;
        self.sparse.get(position).map(|entry| &entry.slot)
    }

    fn slot_mut(&mut self, index: u32) -> Option<&mut Slot<T>> {
        if usize::try_from(index).ok()? < self.slots.len() {
            return self.slots.get_mut(index as usize);
        }
        let position = self
            .sparse
            .binary_search_by_key(&index, |entry| entry.index)
            .ok()?;
        self.sparse.get_mut(position).map(|entry| &mut entry.slot)
    }

    fn remove_free(&mut self, index: u32) -> Option<()> {
        let position = self.free.iter().position(|&row| row == index)?;
        self.free.swap_remove(position);
        Some(())
    }

    fn densify_sparse_prefix(&mut self) {
        let mut count = 0usize;
        let mut expected = crate::column::saturating_u32(self.slots.len());
        for entry in &self.sparse {
            if entry.index != expected {
                break;
            }
            count = count.saturating_add(1);
            expected = expected.saturating_add(1);
        }
        if count == 0 {
            return;
        }
        self.slots
            .extend(self.sparse.drain(..count).map(|entry| entry.slot));
    }
}
