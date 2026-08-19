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

/// Identifies one immutable caller-supplied atom property column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct AtomPropertyHandle(pub(crate) RawHandle);

/// Identifies one caller-declared weighted structural ensemble.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct EnsembleHandle(pub(crate) RawHandle);

/// Identifies one caller-supplied analytic primitive.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PrimitiveHandle(pub(crate) RawHandle);

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
    AtomPropertyHandle,
    PrimitiveHandle,
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
    free: Vec<u32>,
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

impl<T> SlotMap<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Reserves backing slots for an insertion batch without changing handles.
    pub fn reserve(&mut self, additional: usize) {
        self.slots.reserve(additional);
    }

    /// Inserts a value, reusing a freed slot when one exists.
    pub fn insert(&mut self, value: T) -> RawHandle {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.value = Some(value);
            return RawHandle {
                index,
                generation: slot.generation,
            };
        }
        let index = crate::column::saturating_u32(self.slots.len());
        self.slots.push(Slot {
            generation: 0,
            value: Some(value),
        });
        RawHandle {
            index,
            generation: 0,
        }
    }

    /// Inserts a value at a previously serialized identity.
    ///
    /// This is intentionally separate from [`Self::insert`]: normal scene
    /// edits allocate the next available identity, while rehydration must
    /// preserve rows and generations from a trusted manifest. Empty slots
    /// between zero and `handle.index` are retained as reusable holes.
    pub fn insert_at(&mut self, handle: RawHandle, value: T) -> Option<()> {
        let index = usize::try_from(handle.index).ok()?;
        while self.slots.len() <= index {
            let row = crate::column::saturating_u32(self.slots.len());
            self.slots.push(Slot {
                generation: 0,
                value: None,
            });
            self.free.push(row);
        }
        let slot = self.slots.get_mut(index)?;
        if slot.value.is_some() {
            return None;
        }
        slot.generation = handle.generation;
        slot.value = Some(value);
        let free_index = self.free.iter().position(|&row| row == handle.index)?;
        self.free.swap_remove(free_index);
        Some(())
    }

    /// Resolves a handle, or `None` when it is stale.
    pub fn get(&self, handle: RawHandle) -> Option<&T> {
        let slot = self.slots.get(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.value.as_ref()
    }

    /// Resolves a live row and returns its current generation-checked handle.
    pub fn get_index(&self, index: u32) -> Option<(RawHandle, &T)> {
        let slot = self.slots.get(index as usize)?;
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
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.value.as_mut()
    }

    /// Removes an entry, invalidating every handle to it.
    pub fn remove(&mut self, handle: RawHandle) -> Option<T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(handle.index);
        Some(value)
    }

    /// Iterates live entries in slot order, which is stable across removals
    /// of other entries.
    pub fn iter(&self) -> impl Iterator<Item = (RawHandle, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, slot)| {
            let value = slot.value.as_ref()?;
            let index = crate::column::saturating_u32(i);
            Some((
                RawHandle {
                    index,
                    generation: slot.generation,
                },
                value,
            ))
        })
    }

    /// Number of live entries.
    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }
}
