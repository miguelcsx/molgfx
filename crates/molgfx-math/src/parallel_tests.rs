//! The property that matters here is not speed but that the answer does not
//! move. Every test runs the same input through pools of different sizes and
//! demands byte-identical output, because a reduction whose result depends on
//! how many cores were free is not reproducible and cannot back a golden image.
//!
//! The index-to-float and index-to-`u32` casts below build fixtures from loop
//! counters whose ranges are fixed literals in this file; there is nothing to
//! truncate.

use super::{
    BLOCK, all_blocks, for_each_block, map_blocks_into, map_zip_blocks_into, reduce_blocks,
};
use num_traits::ToPrimitive as _;

fn pool(threads: usize) -> rayon::ThreadPool {
    match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
        Ok(pool) => pool,
        Err(error) => panic!("thread pool of {threads} must build: {error}"),
    }
}

/// A float column whose summation order is observable: values of wildly
/// different magnitude, so a regrouped sum would land somewhere else.
fn ill_conditioned(len: usize) -> Vec<f32> {
    (0..len)
        .map(|index| {
            if index % 3 == 0 {
                1.0e7
            } else {
                1.0e-3 * (index % 7).to_f32().expect("fixture index fits f32")
            }
        })
        .collect()
}

#[test]
fn a_float_reduction_returns_the_same_bits_at_every_thread_count() {
    let values = ill_conditioned(BLOCK * 5 + 137);
    let run = || {
        reduce_blocks(
            &values,
            0,
            0.0f32,
            |block| block.iter().sum::<f32>(),
            |left, right| left + right,
        )
    };
    let single = pool(1).install(run);
    for threads in [2usize, 3, 8, 16] {
        assert_eq!(
            pool(threads).install(run).to_bits(),
            single.to_bits(),
            "{threads} threads disagreed with one"
        );
    }
}

#[test]
fn a_parallel_reduction_matches_the_serial_one_below_the_threshold() {
    let values = ill_conditioned(BLOCK * 3 + 11);
    let fold = |block: &[f32]| block.iter().sum::<f32>();
    let combine = |left: f32, right: f32| left + right;
    // A min_len above the input length forces the serial path.
    let serial = reduce_blocks(&values, usize::MAX, 0.0, fold, combine);
    let parallel = pool(8).install(|| reduce_blocks(&values, 0, 0.0, fold, combine));
    assert_eq!(serial.to_bits(), parallel.to_bits());
}

#[test]
fn a_block_map_writes_every_element_exactly_once() {
    for len in [0usize, 1, BLOCK - 1, BLOCK, BLOCK + 1, BLOCK * 3 + 7] {
        let end = u32::try_from(len).expect("fixture length fits u32");
        let src: Vec<u32> = (0..end).collect();
        let mut dst = vec![0u32; len];
        pool(8).install(|| {
            map_blocks_into(&src, &mut dst, 0, |source, target| {
                for (value, slot) in source.iter().zip(target) {
                    *slot = value.wrapping_mul(3).wrapping_add(1);
                }
            });
        });
        let expected: Vec<u32> = src
            .iter()
            .map(|v| v.wrapping_mul(3).wrapping_add(1))
            .collect();
        assert_eq!(dst, expected, "length {len}");
    }
}

#[test]
fn a_block_map_truncates_to_the_shorter_of_its_two_slices() {
    let src: Vec<u32> = (0..10).collect();
    let mut dst = vec![0u32; 4];
    map_blocks_into(&src, &mut dst, 0, |source, target| {
        target.copy_from_slice(source);
    });
    assert_eq!(dst, vec![0, 1, 2, 3]);
}

#[test]
fn a_zipped_block_map_matches_the_serial_form_at_every_thread_count() {
    for len in [0usize, 1, BLOCK - 1, BLOCK, BLOCK * 2 + 9] {
        let end = u32::try_from(len).expect("fixture length fits u32");
        let left: Vec<f32> = (0..end)
            .map(|value| value.to_f32().expect("fixture value fits f32"))
            .collect();
        let right = left.clone();
        let expected: Vec<f32> = left
            .iter()
            .zip(&right)
            .map(|(&a, &b)| a.mul_add(2.0, b))
            .collect();
        let mut dst = vec![0.0f32; len];
        pool(8).install(|| {
            map_zip_blocks_into(&left, &right, &mut dst, 0, |a, b, output| {
                for ((slot, &x), &y) in output.iter_mut().zip(a).zip(b) {
                    *slot = x.mul_add(2.0, y);
                }
            });
        });
        for (got, want) in dst.iter().zip(&expected) {
            assert_eq!(got.to_bits(), want.to_bits(), "length {len}");
        }
    }
}

#[test]
fn every_block_is_visited_with_its_own_start_offset() {
    for len in [0usize, 1, BLOCK, BLOCK * 2 + 5] {
        let values: Vec<usize> = (0..len).collect();
        let seen = std::sync::Mutex::new(Vec::new());
        pool(4).install(|| {
            for_each_block(&values, 0, |offset, block| {
                match seen.lock() {
                    Ok(mut seen) => seen.push((offset, block.len())),
                    Err(error) => panic!("block record must lock: {error}"),
                }
                assert_eq!(
                    block[0], offset,
                    "block at {offset} started at the wrong row"
                );
            });
        });
        let mut seen = match seen.into_inner() {
            Ok(seen) => seen,
            Err(error) => panic!("block record must unlock: {error}"),
        };
        seen.sort_unstable();
        let total: usize = seen.iter().map(|(_, count)| count).sum();
        assert_eq!(total, len, "length {len} did not cover every row");
    }
}

#[test]
fn an_all_predicate_agrees_with_the_serial_form_and_finds_a_single_failure() {
    let len = BLOCK * 2 + 3;
    let end = u32::try_from(len).expect("fixture length fits u32");
    let values: Vec<u32> = (0..end).collect();
    assert!(all_blocks(&values, 0, |block| block
        .iter()
        .all(|v| *v < end)));
    for position in [0usize, BLOCK, BLOCK + 1, len - 1] {
        let mut values = values.clone();
        values[position] = u32::MAX;
        assert!(
            !pool(8).install(|| all_blocks(&values, 0, |block| block.iter().all(|v| *v < end))),
            "failure at {position} went unseen"
        );
    }
}

#[test]
fn an_empty_input_reduces_to_the_identity_without_touching_the_pool() {
    let empty: [f32; 0] = [];
    let total = reduce_blocks(&empty, 0, 7.5f32, |block| block.iter().sum(), |a, b| a + b);
    assert_eq!(total.to_bits(), 7.5f32.to_bits());
    assert!(all_blocks(&empty, 0, |_| false));
}

#[test]
fn helpers_use_the_callers_shared_rayon_pool() {
    let values = vec![1u32; BLOCK * 2];
    let observed_worker = std::sync::atomic::AtomicBool::new(false);
    pool(2).install(|| {
        for_each_block(&values, 0, |_, _| {
            observed_worker.store(
                rayon::current_thread_index().is_some(),
                std::sync::atomic::Ordering::Relaxed,
            );
        });
    });
    assert!(observed_worker.load(std::sync::atomic::Ordering::Relaxed));
}
