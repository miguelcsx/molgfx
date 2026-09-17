//! Portable two-dimensional compute dispatch sizing.

const MAX_DISPATCH_AXIS: u64 = 65_535;

/// Maps a linear workgroup count onto portable WebGPU X/Y limits.
pub(super) fn workgroups_2d(total: u64) -> [u32; 2] {
    if total == 0 {
        return [0, 0];
    }
    // Choose Y first, then the shortest X that covers the linear workload.
    // This avoids nearly doubling the dispatch at MAX_DISPATCH_AXIS + 1 while
    // preserving a dense row-major index in the shader.
    let y = total.div_ceil(MAX_DISPATCH_AXIS);
    let x = total.div_ceil(y);
    debug_assert!(x <= MAX_DISPATCH_AXIS && y <= MAX_DISPATCH_AXIS);
    [
        u32::try_from(x).map_or(u32::MAX, |value| value),
        u32::try_from(y).map_or(u32::MAX, |value| value),
    ]
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
