//! Ramps that read a per-atom confidence score.

use super::{NamedRamp, RampKind};

pub(super) const RAMPS: &[NamedRamp] = &[
    // The four AlphaFold confidence bands (very low below 50, low to 70,
    // confident to 90, very high above) laid over a 0-100 domain. Eleven
    // evenly spaced anchors blend each band edge over ten score points, so a
    // score of 45 is a mix of the two neighbouring bands rather than a step.
    NamedRamp {
        name: "plddt",
        kind: RampKind::Sequential,
        anchors: &[
            0xff_7d_45, 0xff_7d_45, 0xff_7d_45, 0xff_7d_45, 0xff_7d_45, 0xff_db_13, 0xff_db_13,
            0x65_cb_f3, 0x65_cb_f3, 0x00_53_d6, 0x00_53_d6,
        ],
    },
];
