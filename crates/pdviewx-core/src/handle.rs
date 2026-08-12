//! Generational handles and the slot map behind them.
//!
//! A handle stays valid across unrelated edits and detects its own staleness:
//! removing an entry bumps the slot's generation, so a handle held from
//! before the removal no longer resolves. All operations are `O(1)`.

#[cfg(test)]
#[path = "handle_tests.rs"]
mod tests;

/// Identifies a placed structure within a scene.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct StructureHandle(pub(crate) RawHandle);

/// Identifies a representation within a scene.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RepresentationHandle(pub(crate) RawHandle);

/// Identifies a stored selection within a scene.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SelectionHandle(pub(crate) RawHandle);

/// Slot index plus generation; the unit every typed handle wraps.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RawHandle {
    index: u32,
    generation: u32,
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

    /// Resolves a handle, or `None` when it is stale.
    pub fn get(&self, handle: RawHandle) -> Option<&T> {
        let slot = self.slots.get(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.value.as_ref()
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
