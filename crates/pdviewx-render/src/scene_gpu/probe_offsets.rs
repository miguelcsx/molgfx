//! Deterministic rolling-probe sample directions for solvent-excluded erosion.
//!
//! The solvent-excluded surface is the boundary a probe of the caller's radius
//! reaches without overlapping an atom. The erosion pass rolls that probe by
//! sampling the inflated field at a fixed set of directions around each voxel;
//! this module produces those directions. They are a Fibonacci sphere — the
//! evenly spread point set with no clustering at the poles — scaled by the
//! probe radius, so the roll is isotropic and its cost is a fixed sample count
//! regardless of grid size. The sequence is fixed, so the surface is
//! reproducible frame to frame and across runs.

/// The number of rolling-probe directions.
///
/// This matches the sample count the erosion shader reads from the surface
/// uniform, so every generated direction is used and none is skipped. It is a
/// `u8` so the loop counter converts to `f32` without loss.
pub(super) const PROBE_SAMPLE_COUNT: u8 = 32;

/// The direction count as an array length.
pub(super) const PROBE_SAMPLE_LEN: usize = PROBE_SAMPLE_COUNT as usize;

/// Returns `PROBE_SAMPLE_COUNT` probe offsets, each a unit Fibonacci-sphere
/// direction scaled by `probe`. The fourth lane is unused padding.
///
/// The set is small and fixed, so it lives on the stack and is returned by
/// value for a direct upload — generation allocates nothing.
pub(super) fn probe_offsets(probe: f32) -> [[f32; 4]; PROBE_SAMPLE_LEN] {
    // The golden angle, pi*(3 - sqrt(5)) in radians, is the turn per step that
    // spreads the points evenly; the vertical stride marches from pole to pole.
    const GOLDEN_ANGLE: f32 = 2.399_963_2;
    let mut offsets = [[0.0_f32; 4]; PROBE_SAMPLE_LEN];
    let last = f32::from(PROBE_SAMPLE_COUNT - 1);
    for index in 0..PROBE_SAMPLE_COUNT {
        let step = f32::from(index);
        let y = 1.0 - (step / last) * 2.0;
        let radius = (1.0 - y * y).max(0.0).sqrt();
        let theta = GOLDEN_ANGLE * step;
        offsets[usize::from(index)] = [
            theta.cos() * radius * probe,
            y * probe,
            theta.sin() * radius * probe,
            0.0,
        ];
    }
    offsets
}

#[cfg(test)]
#[path = "probe_offsets_tests.rs"]
mod tests;
