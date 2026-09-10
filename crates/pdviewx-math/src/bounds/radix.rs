//! Stable radix sort over Morton keys.
//!
//! A comparison sort costs `O(n log n)` key comparisons; at a billion
//! primitives that is thirty billion branches on a key that is already an
//! integer. Radix sorting the same key is `O(n · bytes)` with no comparisons at
//! all, and the byte count is fixed, so the cost is linear in the input.
//!
//! **Stability is load-bearing, not incidental.** Entries arrive in ascending
//! source order, so a stable sort by code alone reproduces the `(code, source)`
//! ordering the hierarchy is specified against, without carrying the source
//! through the key. Every pass here is a counting sort, which is stable by
//! construction.
//!
//! Parallelism comes from the top byte: one stable partition splits the input
//! into 256 contiguous buckets that share no memory, and each bucket then sorts
//! its remaining bytes independently. That keeps the whole sort inside safe
//! Rust — a classic parallel scatter needs disjointness the borrow checker
//! cannot see, while this crate permits no unchecked code.
//!
//! Cost is `O(n · 8 / threads)` above the threshold and `O(n · 8)` below it,
//! with one scratch buffer the caller retains across rebuilds.

use super::bvh::MortonEntry;
use rayon::prelude::*;

/// Buckets per pass; one byte of the key.
const RADIX: usize = 256;

/// Bytes in a Morton key.
const BYTES: usize = 8;

/// Elements per histogram block. Large enough that 256 counters per block stay
/// a rounding error against the data, small enough to keep every core fed.
const HISTOGRAM_BLOCK: usize = 1 << 16;

/// Sorts `entries` in place by `code`, preserving the relative order of equal
/// keys. `scratch` is reused across calls and left with the same length.
pub(super) fn sort_by_code(
    entries: &mut Vec<MortonEntry>,
    scratch: &mut Vec<MortonEntry>,
    min_len: usize,
) {
    if entries.len() < 2 {
        return;
    }
    if entries.len() < min_len {
        sort_serial(entries, scratch, 0..BYTES);
        return;
    }
    let counts = histogram(entries, BYTES - 1);
    let Some(offsets) = single_bucket_check(&counts) else {
        // Every key shares a top byte, so the partition would be a copy. Sort
        // the remaining bytes over the whole slice instead.
        sort_serial(entries, scratch, 0..BYTES - 1);
        return;
    };
    scatter(entries, scratch, BYTES - 1, offsets);
    std::mem::swap(entries, scratch);
    sort_buckets(entries, &counts);
}

/// Per-bucket counts of one byte of the key.
fn histogram(entries: &[MortonEntry], byte: usize) -> [usize; RADIX] {
    entries
        .par_chunks(HISTOGRAM_BLOCK)
        .map(|block| {
            let mut counts = [0usize; RADIX];
            for entry in block {
                counts[digit(entry.code, byte)] += 1;
            }
            counts
        })
        .reduce(
            || [0usize; RADIX],
            |mut total, part| {
                for (total, part) in total.iter_mut().zip(part) {
                    *total += part;
                }
                total
            },
        )
}

/// Exclusive prefix sums, or `None` when one bucket holds everything.
fn single_bucket_check(counts: &[usize; RADIX]) -> Option<[usize; RADIX]> {
    if counts.iter().filter(|count| **count != 0).count() <= 1 {
        return None;
    }
    let mut offsets = [0usize; RADIX];
    let mut running = 0usize;
    for (offset, count) in offsets.iter_mut().zip(counts) {
        *offset = running;
        running += count;
    }
    Some(offsets)
}

/// One stable counting-sort pass from `source` into `target`.
fn scatter(
    source: &[MortonEntry],
    target: &mut Vec<MortonEntry>,
    byte: usize,
    mut offsets: [usize; RADIX],
) {
    target.clear();
    target.resize(source.len(), MortonEntry::PLACEHOLDER);
    for entry in source {
        let bucket = digit(entry.code, byte);
        // Every offset was derived from the histogram of this same slice, so a
        // bucket cannot run past its allotted range.
        if let Some(slot) = target.get_mut(offsets[bucket]) {
            *slot = *entry;
        }
        offsets[bucket] += 1;
    }
}

/// Sorts each contiguous top-byte bucket by the remaining bytes, in parallel.
fn sort_buckets(entries: &mut [MortonEntry], counts: &[usize; RADIX]) {
    let mut buckets: Vec<&mut [MortonEntry]> = Vec::with_capacity(RADIX);
    let mut rest = entries;
    for count in counts {
        let take = (*count).min(rest.len());
        let (bucket, tail) = rest.split_at_mut(take);
        if bucket.len() > 1 {
            buckets.push(bucket);
        }
        rest = tail;
    }
    buckets.into_par_iter().for_each(|bucket| {
        let mut scratch = Vec::new();
        sort_serial(bucket, &mut scratch, 0..BYTES - 1);
    });
}

/// Stable least-significant-digit radix sort over `bytes`, ping-ponging
/// between `entries` and `scratch`. Passes whose byte is constant are skipped,
/// which is the common case for the high bytes of a Morton key.
fn sort_serial(
    entries: &mut [MortonEntry],
    scratch: &mut Vec<MortonEntry>,
    bytes: std::ops::Range<usize>,
) {
    if entries.len() < 2 {
        return;
    }
    scratch.clear();
    scratch.resize(entries.len(), MortonEntry::PLACEHOLDER);
    // `flipped` tracks which buffer currently holds the live data.
    let mut flipped = false;
    for byte in bytes {
        let counts = {
            let source: &[MortonEntry] = if flipped { scratch } else { entries };
            let mut counts = [0usize; RADIX];
            for entry in source {
                counts[digit(entry.code, byte)] += 1;
            }
            counts
        };
        let Some(mut offsets) = single_bucket_check(&counts) else {
            continue;
        };
        if flipped {
            for entry in scratch.iter() {
                let bucket = digit(entry.code, byte);
                if let Some(slot) = entries.get_mut(offsets[bucket]) {
                    *slot = *entry;
                }
                offsets[bucket] += 1;
            }
        } else {
            for entry in entries.iter() {
                let bucket = digit(entry.code, byte);
                if let Some(slot) = scratch.get_mut(offsets[bucket]) {
                    *slot = *entry;
                }
                offsets[bucket] += 1;
            }
        }
        flipped = !flipped;
    }
    if flipped {
        entries.copy_from_slice(scratch);
    }
}

/// One byte of the key, least significant first.
#[inline]
fn digit(code: u64, byte: usize) -> usize {
    ((code >> (byte * 8)) & 0xff) as usize
}

#[cfg(test)]
#[path = "radix_tests.rs"]
mod tests;
