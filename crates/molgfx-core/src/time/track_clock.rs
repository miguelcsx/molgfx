//! A timeline track's position as a structure's visual programs read it.

/// Where one structure's timeline track stands, kept in `f64`.
///
/// `phase` is the fraction of the track's own simulation interval already
/// played. `display_time` is the track's physical time expressed in the track's
/// display unit (nanoseconds, picoseconds and so on), so a value that would be
/// `1e-15` seconds reads as a number near one. Both stay exact on the CPU and
/// are narrowed to `f32` only when the uniform is written. A structure no
/// timeline drives reads zero for both.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct TrackClock {
    /// Fraction of the track's interval played, in `[0, 1]`.
    pub phase: f64,
    /// The track's physical time, in its display unit.
    pub display_time: f64,
}
