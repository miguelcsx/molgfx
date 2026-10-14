//! The presentation clock that viewers and frame writers advance.

/// A position on the presentation clock, in seconds.
///
/// Playback time is what a viewer shows and what a frame writer samples: frame
/// `i` of a sequence at `fps` is `i / fps` on this clock. It is kept distinct
/// from [`SimTime`](super::SimTime), which records the physical time a
/// simulation covered. A timeline track maps one onto the other at its own
/// rate, so the type system refuses to hand a physical time to a transport.
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct PlaybackTime {
    seconds: f64,
}

impl PlaybackTime {
    /// Canonical seconds on the presentation clock.
    #[must_use]
    pub const fn seconds(self) -> f64 {
        self.seconds
    }
}

impl From<f64> for PlaybackTime {
    /// Reads a bare number as seconds on the presentation clock.
    fn from(seconds: f64) -> Self {
        Self { seconds }
    }
}
