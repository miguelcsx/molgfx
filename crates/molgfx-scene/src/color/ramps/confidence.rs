//! Ramps that read a per-atom confidence score.

use super::{NamedRamp, RampKind};

const VERY_LOW: u32 = 0xff_7d_45;
const LOW: u32 = 0xff_db_13;
const CONFIDENT: u32 = 0x65_cb_f3;
const VERY_HIGH: u32 = 0x00_53_d6;

/// One published confidence band of a per-atom score on a 0-100 scale.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConfidenceBand {
    /// The band as the source publishes it, for a legend.
    pub label: &'static str,
    /// The score at which the band begins, inclusive.
    pub lower: u8,
    /// The band's colour, the one the `plddt` ramp is anchored on.
    pub rgb: [u8; 3],
}

const fn rgb(packed: u32) -> [u8; 3] {
    let [_, red, green, blue] = packed.to_be_bytes();
    [red, green, blue]
}

/// The four `AlphaFold` confidence bands, very high first. The `plddt` ramp is
/// anchored on the same colours, so a legend of bands and a blended colouring
/// cannot disagree.
pub const PLDDT_BANDS: [ConfidenceBand; 4] = [
    ConfidenceBand {
        label: "Very high (>90)",
        lower: 90,
        rgb: rgb(VERY_HIGH),
    },
    ConfidenceBand {
        label: "Confident (70-90)",
        lower: 70,
        rgb: rgb(CONFIDENT),
    },
    ConfidenceBand {
        label: "Low (50-70)",
        lower: 50,
        rgb: rgb(LOW),
    },
    ConfidenceBand {
        label: "Very low (<50)",
        lower: 0,
        rgb: rgb(VERY_LOW),
    },
];

pub(super) const RAMPS: &[NamedRamp] = &[
    // The four AlphaFold confidence bands (very low below 50, low to 70,
    // confident to 90, very high above) laid over a 0-100 domain. Eleven
    // evenly spaced anchors blend each band edge over ten score points, so a
    // score of 45 is a mix of the two neighbouring bands rather than a step.
    NamedRamp {
        name: "plddt",
        kind: RampKind::Sequential,
        anchors: &[
            VERY_LOW, VERY_LOW, VERY_LOW, VERY_LOW, VERY_LOW, LOW, LOW, CONFIDENT, CONFIDENT,
            VERY_HIGH, VERY_HIGH,
        ],
    },
];
