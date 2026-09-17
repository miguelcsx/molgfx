//! The transient texture pool.
//!
//! Transients are described in the graph and allocated here. Two resources
//! whose scheduled lifetimes do not overlap share one physical texture
//! (classic interval aliasing), and physical textures persist across frames:
//! nothing in the pool is freed and recreated inside the frame loop.
//! Assignment is `O(resources²)` at graph build; lookups at record time are
//! `O(1)`.

use crate::graph::node::{PassNode, ResourceDesc, ResourceId, SizeClass};
use molgfx_gpu::{Device, GpuError, TextureDesc};

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "pool_tests.rs"]
mod tests;

/// A physical slot key: resources may share a slot only when the concrete
/// texture would be identical.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct SlotKey {
    format: molgfx_gpu::TextureFormat,
    size: SizeClass,
    usage: u32,
}

#[derive(Clone)]
struct Slot {
    key: SlotKey,
    // Occupied lifetime steps, as (first, last) intervals.
    intervals: Vec<(usize, usize)>,
}

/// The aliasing plan: which physical slot each declared resource uses.
/// Pure data, computed without a device, so it is testable directly.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AliasPlan {
    /// `slot[i]` is the physical slot index of resource `i`, or `usize::MAX`
    /// when no scheduled pass uses the resource.
    pub slot: Vec<usize>,
    /// Number of physical slots.
    pub slots: usize,
}

/// Computes lifetimes from the schedule and greedily assigns compatible,
/// non-overlapping resources to shared slots.
pub fn plan_aliases<D: Device>(
    resources: &[ResourceDesc],
    passes: &[PassNode<D>],
    order: &[usize],
) -> AliasPlan {
    let n = resources.len();
    // Lifetime of resource r = [first step touching r, last step touching r].
    let mut first = vec![usize::MAX; n];
    let mut last = vec![0usize; n];
    for (step, &pass_index) in order.iter().enumerate() {
        let Some(pass) = passes.get(pass_index) else {
            continue;
        };
        for id in pass.reads.iter().chain(pass.writes.iter()) {
            let r = id.0 as usize;
            if r < n {
                first[r] = first[r].min(step);
                last[r] = last[r].max(step);
            }
        }
    }

    let mut slots: Vec<Slot> = Vec::new();
    let mut assignment = vec![usize::MAX; n];

    for r in 0..n {
        let Some(desc) = resources.get(r) else {
            continue;
        };
        if first[r] == usize::MAX {
            continue;
        }
        let key = SlotKey {
            format: desc.format,
            size: desc.size,
            usage: desc.usage.bits(),
        };
        let lo = if desc.persistent { 0 } else { first[r] };
        let hi = if desc.persistent {
            order.len().saturating_sub(1)
        } else {
            last[r]
        };
        let found = slots
            .iter_mut()
            .enumerate()
            .find(|(_, s)| s.key == key && s.intervals.iter().all(|&(a, b)| hi < a || lo > b));
        if let Some((index, slot)) = found {
            slot.intervals.push((lo, hi));
            assignment[r] = index;
        } else {
            slots.push(Slot {
                key,
                intervals: vec![(lo, hi)],
            });
            assignment[r] = slots.len() - 1;
        }
    }

    AliasPlan {
        slot: assignment,
        slots: slots.len(),
    }
}

/// The physical textures backing a plan at one frame size. Rebuilt only
/// when the frame size or the graph changes.
#[derive(Debug)]
pub struct TransientPool<D: Device> {
    plan: AliasPlan,
    /// One (texture, view) per physical slot, in slot order.
    textures: Vec<(D::Texture, D::TextureView)>,
    width: u32,
    height: u32,
}

impl<D: Device> TransientPool<D> {
    /// Builds the physical pool for a plan at a frame size.
    ///
    /// # Errors
    ///
    /// Texture creation exceeded device limits.
    pub fn build(
        device: &D,
        resources: &[ResourceDesc],
        plan: AliasPlan,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        // The first resource mapped to each slot defines its concrete desc.
        let mut slot_desc: Vec<Option<ResourceDesc>> = vec![None; plan.slots];
        for (r, &slot) in plan.slot.iter().enumerate() {
            let Some(desc) = slot_desc.get_mut(slot) else {
                continue;
            };
            if desc.is_none() {
                *desc = resources.get(r).copied();
            }
        }
        let mut textures = Vec::with_capacity(plan.slots);
        for desc in slot_desc.into_iter().flatten() {
            let (w, h) = match desc.size {
                SizeClass::Full => (width, height),
                SizeClass::Tiles16 => (width.div_ceil(16), height.div_ceil(16)),
                SizeClass::Shadow => (2048, 2048),
                SizeClass::Quarter => (width.div_ceil(4).max(1), height.div_ceil(4).max(1)),
            };
            let texture = device.create_texture(&TextureDesc {
                label: desc.label,
                width: w,
                height: h,
                depth: 1,
                dimension: molgfx_gpu::TextureDimension::D2,
                format: desc.format,
                usage: desc.usage,
            })?;
            let view =
                device.create_texture_view(&texture, &molgfx_gpu::TextureViewDesc::default());
            textures.push((texture, view));
        }
        Ok(Self {
            plan,
            textures,
            width,
            height,
        })
    }

    /// The view backing a declared resource.
    pub fn view(&self, id: ResourceId) -> Option<&D::TextureView> {
        let slot = *self.plan.slot.get(id.0 as usize)?;
        self.textures.get(slot).map(|(_, view)| view)
    }

    /// The physical texture backing a declared resource.
    pub fn texture(&self, id: ResourceId) -> Option<&D::Texture> {
        let slot = *self.plan.slot.get(id.0 as usize)?;
        self.textures.get(slot).map(|(texture, _)| texture)
    }

    /// Whether this pool matches the given frame size.
    pub fn matches(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }
}
