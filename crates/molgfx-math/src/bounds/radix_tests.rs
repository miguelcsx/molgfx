//! Radix sorting is only a drop-in if it agrees with the comparison sort it
//! replaced on both the ordering and the tie-breaking. These check the second
//! part especially: equal keys must keep their input order, because that is
//! what stands in for the source component of the key.
//!
//! The index-to-`u32` casts build fixtures from loop counters bounded by
//! literals in this file; there is nothing to truncate.

use super::super::bvh::MortonEntry;
use super::sort_by_code;

fn entries(codes: &[u64]) -> Vec<MortonEntry> {
    codes
        .iter()
        .enumerate()
        .map(|(source, code)| MortonEntry {
            code: *code,
            source: u32::try_from(source).expect("fixture index fits u32"),
        })
        .collect()
}

fn reference(codes: &[u64]) -> Vec<(u64, u32)> {
    let mut expected: Vec<(u64, u32)> = codes
        .iter()
        .enumerate()
        .map(|(source, code)| {
            (
                *code,
                u32::try_from(source).expect("fixture index fits u32"),
            )
        })
        .collect();
    expected.sort_unstable();
    expected
}

fn check(codes: &[u64], min_len: usize) {
    let mut values = entries(codes);
    let mut scratch = Vec::new();
    sort_by_code(&mut values, &mut scratch, min_len);
    let actual: Vec<(u64, u32)> = values.iter().map(|e| (e.code, e.source)).collect();
    assert_eq!(
        actual,
        reference(codes),
        "min_len {min_len}, {} keys",
        codes.len()
    );
}

/// A deterministic spread with heavy duplication, so the stability path is
/// exercised rather than incidentally satisfied by unique keys.
fn spread(len: usize, distinct: u64) -> Vec<u64> {
    let mut codes = Vec::with_capacity(len);
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..len {
        state ^= state >> 30;
        state = state.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        state ^= state >> 27;
        codes.push((state % distinct) & 0x7fff_ffff_ffff_ffff);
    }
    codes
}

#[test]
fn sorting_agrees_with_the_comparison_sort_on_both_paths() {
    for len in [0usize, 1, 2, 3, 17, 255, 256, 257, 4096] {
        let codes = spread(len, 1_000_000);
        check(&codes, 0);
        check(&codes, usize::MAX);
    }
}

#[test]
fn equal_keys_keep_their_input_order_so_the_source_never_needs_carrying() {
    // Ten distinct keys across a thousand entries: every key repeats ~100 times.
    let codes = spread(1000, 10);
    check(&codes, 0);
    check(&codes, usize::MAX);
}

#[test]
fn a_column_of_one_repeated_key_is_left_in_source_order() {
    let codes = vec![42u64; 500];
    check(&codes, 0);
    check(&codes, usize::MAX);
}

#[test]
fn keys_that_differ_only_in_the_top_byte_partition_correctly() {
    let codes: Vec<u64> = (0..512u64).map(|index| (index % 256) << 48).collect();
    check(&codes, 0);
    check(&codes, usize::MAX);
}

#[test]
fn keys_that_differ_only_in_the_lowest_byte_still_sort() {
    let codes: Vec<u64> = (0..512u64).map(|index| index % 256).collect();
    check(&codes, 0);
    check(&codes, usize::MAX);
}

#[test]
fn the_parallel_and_serial_paths_produce_the_same_permutation() {
    let codes = spread(20_000, 5_000);
    let mut parallel = entries(&codes);
    let mut serial = entries(&codes);
    let mut scratch = Vec::new();
    sort_by_code(&mut parallel, &mut scratch, 0);
    sort_by_code(&mut serial, &mut scratch, usize::MAX);
    let parallel: Vec<(u64, u32)> = parallel.iter().map(|e| (e.code, e.source)).collect();
    let serial: Vec<(u64, u32)> = serial.iter().map(|e| (e.code, e.source)).collect();
    assert_eq!(parallel, serial);
}

#[test]
fn the_full_63_bit_range_sorts_without_losing_high_bits() {
    let codes = vec![
        0x7fff_ffff_ffff_ffff,
        0x0000_0000_0000_0001,
        0x4000_0000_0000_0000,
        0x0000_0000_ffff_ffff,
        0x0000_ffff_0000_0000,
        0,
    ];
    check(&codes, 0);
    check(&codes, usize::MAX);
}
