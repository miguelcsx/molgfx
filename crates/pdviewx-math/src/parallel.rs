//! Threshold-guarded data parallelism with a thread-count-independent shape.
//!
//! Two rules make every helper here safe to drop into a load path that has to
//! stay reproducible.
//!
//! **The partition is a pure function of the input length.** Work is split into
//! fixed-size blocks, never into "one piece per thread", so the same input
//! produces the same blocks on a laptop and on a build machine. A reduction
//! then combines block results in index order. Floating-point addition is not
//! associative, so a thread-count-dependent partition would give a
//! thread-count-dependent answer; a fixed one cannot.
//!
//! **Small inputs stay on the calling thread.** Every entry point takes a
//! minimum length and runs serially below it. A ligand must not pay for a
//! thread pool, and a ribosome must not be denied one.
//!
//! Cost is `O(n / threads)` above the threshold and `O(n)` below it, with one
//! `Vec` of block results — `blocks = n / BLOCK` entries — for the reducing
//! forms and no allocation at all for the in-place forms.
//!
//! Every parallel branch schedules onto Rayon's process-wide registry. No
//! scene, renderer or language binding constructs a private pool, so pdviewx,
//! pdbiox and independent Python `Engine` objects share the same Rust workers
//! instead of multiplying thread counts and scratch memory.

use rayon::prelude::*;

/// Elements per block.
///
/// Sized so one block of the widest element this crate reduces over stays
/// inside a typical L2 slice while remaining large enough that per-block
/// scheduling overhead disappears against the work.
pub const BLOCK: usize = 8192;

/// Applies `f` to each `(offset, block)` of `slice`, in parallel above `min_len`.
///
/// Blocks are visited in an unspecified order, so `f` must not depend on
/// ordering; it receives the block's start offset when it needs to address the
/// source.
pub fn for_each_block<T, F>(slice: &[T], min_len: usize, f: F)
where
    T: Sync,
    F: Fn(usize, &[T]) + Send + Sync,
{
    if slice.len() < min_len {
        for (index, block) in slice.chunks(BLOCK).enumerate() {
            f(index * BLOCK, block);
        }
        return;
    }
    slice
        .par_chunks(BLOCK)
        .enumerate()
        .for_each(|(index, block)| f(index * BLOCK, block));
}

/// Writes `dst` from `src` block-wise, in parallel above `min_len`.
///
/// Each block owns a disjoint output range, so no synchronisation is needed and
/// the result is identical to the serial form byte for byte. `dst` is truncated
/// to `src`'s length if it is longer.
pub fn map_blocks_into<T, U, F>(src: &[T], dst: &mut [U], min_len: usize, f: F)
where
    T: Sync,
    U: Send,
    F: Fn(&[T], &mut [U]) + Send + Sync,
{
    let len = src.len().min(dst.len());
    let (src, dst) = (&src[..len], &mut dst[..len]);
    if len < min_len {
        for (source, target) in src.chunks(BLOCK).zip(dst.chunks_mut(BLOCK)) {
            f(source, target);
        }
        return;
    }
    src.par_chunks(BLOCK)
        .zip(dst.par_chunks_mut(BLOCK))
        .for_each(|(source, target)| f(source, target));
}

/// Reduces `slice` with a fixed block partition and an in-order combine.
///
/// `fold` collapses one block to a partial result; `combine` merges two
/// partials. Because the partition depends only on `slice.len()` and the merge
/// walks blocks in index order, the result does not depend on how many threads
/// ran — which is what lets a float reduction keep a stable answer.
pub fn reduce_blocks<T, A, Fold, Combine>(
    slice: &[T],
    min_len: usize,
    identity: A,
    fold: Fold,
    combine: Combine,
) -> A
where
    T: Sync,
    A: Send + Clone,
    Fold: Fn(&[T]) -> A + Send + Sync,
    Combine: Fn(A, A) -> A,
{
    if slice.len() < min_len {
        return slice.chunks(BLOCK).map(&fold).fold(identity, &combine);
    }
    let partials: Vec<A> = slice.par_chunks(BLOCK).map(&fold).collect();
    partials.into_iter().fold(identity, &combine)
}

/// Whether `f` holds for every element, short-circuiting per block.
///
/// A block that fails stops that block immediately; other blocks already in
/// flight run to completion, which costs nothing on the passing path and keeps
/// the answer independent of scheduling.
pub fn all_blocks<T, F>(slice: &[T], min_len: usize, f: F) -> bool
where
    T: Sync,
    F: Fn(&[T]) -> bool + Send + Sync,
{
    if slice.len() < min_len {
        return slice.chunks(BLOCK).all(&f);
    }
    slice.par_chunks(BLOCK).all(&f)
}

#[cfg(test)]
#[path = "parallel_tests.rs"]
mod tests;
