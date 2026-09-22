//! Total, injective encodings that let appearance state sort as a cache key.
//!
//! `ColorScheme`, `ScalarRamp` and `PropertyAppearance` compare by equality but
//! have no total order, and the shared-record cache is a sorted vector. These
//! encodings give that order: two representations pack to equal words exactly
//! when they pack to equal records.

use super::slot_types::RecordState;
use molgfx_core::RepresentationTarget;

impl Eq for RecordState {}

impl PartialOrd for RecordState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RecordState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        packed(self).cmp(&packed(other))
    }
}

/// The total, injective encoding `Ord` compares.
fn packed(state: &RecordState) -> [u64; 8] {
    [
        pack_target(state.target()),
        u64::from(state.kind()),
        u64::from(state.radius_scale()) << 32 | u64::from(state.bond_radius()),
        u64::from(state.surface()[0]) << 32 | u64::from(state.surface()[1]),
        u64::from(state.surface()[2]) << 32 | u64::from(state.surface()[3]),
        0,
        0,
        0,
    ]
}

fn pack_target(target: RepresentationTarget) -> u64 {
    match target {
        RepresentationTarget::Selection(handle) => {
            let word = mix(FNV_OFFSET, u64::from(handle.row()));
            1 << 62 | mix(word, u64::from(handle.generation()))
        }
        RepresentationTarget::Volume(handle) => {
            let word = mix(FNV_OFFSET, u64::from(handle.row()));
            2 << 62 | mix(word, u64::from(handle.generation()))
        }
        RepresentationTarget::SegmentedVolume(handle) => {
            let word = mix(FNV_OFFSET, u64::from(handle.row()));
            3 << 62 | mix(word, u64::from(handle.generation()))
        }
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 1_099_511_628_211;

fn mix(hash: u64, word: u64) -> u64 {
    (hash ^ word).wrapping_mul(FNV_PRIME)
}
