//! Small finite-safe scalar helpers shared by the render-profile styles.

/// Clamps a finite value to `[0, 1]`; a non-finite input becomes zero.
pub(super) fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Linear interpolation without any finiteness assumption on the endpoints.
pub(super) fn lerp(from: f32, to: f32, weight: f32) -> f32 {
    from + (to - from) * weight
}

/// Clamps a finite value to `[minimum, maximum]`, falling back when non-finite.
pub(super) fn finite_clamp(value: f32, minimum: f32, maximum: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}

/// Snaps a cel band count to zero (off) or an integer count in `[2, 16]`.
pub(super) fn sanitize_bands(levels: f32) -> f32 {
    if levels.is_finite() && levels >= 2.0 {
        levels.min(16.0).floor()
    } else {
        0.0
    }
}
