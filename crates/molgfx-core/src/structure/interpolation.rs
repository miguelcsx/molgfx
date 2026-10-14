//! How a trajectory track blends its resident frames.
//!
//! [`InterpolationKind::Linear`] keeps two frames. [`InterpolationKind::CubicHermite`]
//! also keeps the frames `before` and `after` the active interval, when they exist,
//! so the path is C1 across frame boundaries. Everything here is `f64`; only the
//! GPU upload narrows the resulting weights.

#[cfg(test)]
#[path = "interpolation_tests.rs"]
mod tests;

/// Interpolation applied between the two frames that bracket the sample time.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum InterpolationKind {
    /// Straight line between the bracketing frames. Two resident frames.
    #[default]
    Linear,
    /// Cubic Hermite spline with finite-difference tangents taken from the
    /// neighbouring frames. Up to four resident frames.
    CubicHermite,
}

/// Time gaps that determine the Hermite tangents of one segment, in seconds.
///
/// `before` is the gap from the previous frame to the segment start and `after`
/// the gap from the segment end to the next frame. `None` marks an absent neighbour.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct HermiteGaps {
    pub(super) interval: f64,
    pub(super) before: Option<f64>,
    pub(super) after: Option<f64>,
}

impl HermiteGaps {
    /// Scaled tangents `h * m0` and `h * m1` as weights over `[before, start, end, after]`.
    ///
    /// With `h` the interval and `h_b`, `h_a` the neighbour gaps, the tangent at an
    /// interior frame is the three-point derivative of the parabola through its
    /// neighbours, `m = (h_f * d_b + h_b * d_f) / (h_b + h_f)`, where `h_b`, `h_f` are
    /// the gaps to the previous and next frame and `d_b`, `d_f` the backward and
    /// forward finite differences. Both segments sharing a frame use the same
    /// tangent there, which makes the path C1. It is exact for quadratic
    /// motion and reduces to the central difference on uniform spacing. Without a
    /// neighbour the tangent is the one-sided difference `d_f`, so two absent
    /// neighbours make the spline the straight line.
    fn tangents(self) -> ([f64; 4], [f64; 4]) {
        // Weights over [before, start, end, after] of h * m.
        let start = match self.before {
            Some(gap) => {
                let ratio = self.interval / gap;
                let squared = ratio * ratio;
                let norm = 1.0 / (1.0 + ratio);
                [-squared * norm, squared * norm - norm, norm, 0.0]
            }
            None => [0.0, -1.0, 1.0, 0.0],
        };
        let end = match self.after {
            Some(gap) => {
                let ratio = self.interval / gap;
                let squared = ratio * ratio;
                let norm = 1.0 / (1.0 + ratio);
                [0.0, -norm, norm - squared * norm, squared * norm]
            }
            None => [0.0, -1.0, 1.0, 0.0],
        };
        (start, end)
    }

    /// Bezier inner control points `p0 + T0 / 3` and `p1 - T1 / 3` as weights over
    /// `[before, start, end, after]`.
    pub(super) fn control_weights(self) -> [[f64; 4]; 2] {
        let (start, end) = self.tangents();
        let mut lead = start.map(|weight| weight / 3.0);
        let mut trail = end.map(|weight| -weight / 3.0);
        lead[1] += 1.0;
        trail[2] += 1.0;
        [lead, trail]
    }

    /// Blend weights over `[before, start, end, after]` at the fraction `alpha`.
    ///
    /// They always sum to one, so a constant field stays constant.
    pub(super) fn weights(self, alpha: f64) -> [f64; 4] {
        let (start, end) = self.tangents();
        let squared = alpha * alpha;
        let cubed = squared * alpha;
        let h00 = 2.0 * cubed - 3.0 * squared + 1.0;
        let h10 = cubed - 2.0 * squared + alpha;
        let h01 = -2.0 * cubed + 3.0 * squared;
        let h11 = cubed - squared;
        let mut weights = [0.0; 4];
        for (slot, weight) in weights.iter_mut().enumerate() {
            *weight = h10 * start[slot] + h11 * end[slot];
        }
        weights[1] += h00;
        weights[2] += h01;
        weights
    }
}
