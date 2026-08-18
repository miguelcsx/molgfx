//! Compact GPU lookup tables for categorical label styles.

use bytemuck::Zeroable;
use pdviewx_core::SegmentStyle;

/// Direct indexing stays bounded and cache-friendly for compact label domains.
pub(super) const DIRECT_LABEL_LIMIT: u32 = 1_048_576;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct SegmentStyleGpu {
    pub(super) label: u32,
    pub(super) present: u32,
    pub(super) opacity: f32,
    pub(super) padding: u32,
    pub(super) color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LookupMode {
    Direct,
    Hash,
}

#[derive(Clone, Debug)]
pub(super) struct SegmentLookup {
    entries: Vec<SegmentStyleGpu>,
    mode: LookupMode,
    max_label: u32,
}

impl SegmentLookup {
    pub(super) fn new(styles: &[SegmentStyle]) -> Self {
        let max_label = styles.last().map_or(0, |style| style.label);
        let count = styles.len();
        let direct = count > 0
            && max_label <= super::segmentation_lookup::DIRECT_LABEL_LIMIT
            && u64::from(max_label)
                <= u64::try_from(count).map_or(u64::MAX, |value| value.saturating_mul(4));
        if direct {
            let length = match usize::try_from(max_label)
                .ok()
                .and_then(|value| value.checked_add(1))
            {
                Some(length) => length,
                None => 1,
            };
            let mut entries = vec![SegmentStyleGpu::zeroed(); length];
            for style in styles {
                if let Some(entry) = entries.get_mut(style.label as usize) {
                    *entry = gpu_style(*style);
                }
            }
            Self {
                entries,
                mode: LookupMode::Direct,
                max_label,
            }
        } else {
            let capacity = hash_capacity(count);
            let mut entries = vec![SegmentStyleGpu::zeroed(); capacity];
            let mask = u32::try_from(capacity.saturating_sub(1)).map_or(u32::MAX, |value| value);
            for style in styles {
                let mut index = (hash_label(style.label) & mask) as usize;
                for _ in 0..capacity {
                    let Some(entry) = entries.get_mut(index) else {
                        break;
                    };
                    if entry.present == 0 {
                        *entry = gpu_style(*style);
                        break;
                    }
                    index = (index + 1) & (capacity - 1);
                }
            }
            Self {
                entries,
                mode: LookupMode::Hash,
                max_label,
            }
        }
    }

    pub(super) fn entries(&self) -> &[SegmentStyleGpu] {
        &self.entries
    }

    pub(super) const fn mode(&self) -> LookupMode {
        self.mode
    }

    pub(super) const fn max_label(&self) -> u32 {
        self.max_label
    }
}

fn gpu_style(style: SegmentStyle) -> SegmentStyleGpu {
    let color = style.color.to_f32();
    SegmentStyleGpu {
        label: style.label,
        present: 1,
        opacity: style.opacity,
        padding: 0,
        color,
    }
}

fn hash_capacity(count: usize) -> usize {
    let required = count.saturating_mul(2).max(1);
    let mut capacity = 1usize;
    while capacity < required {
        let Some(next) = capacity.checked_mul(2) else {
            return capacity;
        };
        capacity = next;
    }
    capacity
}

pub(super) fn hash_label(label: u32) -> u32 {
    let mut hash = label.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    hash = ((hash >> ((hash >> 28) + 4)) ^ hash).wrapping_mul(277_803_737);
    (hash >> 22) ^ hash
}
