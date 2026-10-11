//! Hue sweeps for ordered positions such as sequence order.

use super::{NamedRamp, RampKind};

const fn ramp(name: &'static str, anchors: &'static [u32]) -> NamedRamp {
    NamedRamp {
        name,
        kind: RampKind::Spectral,
        anchors,
    }
}

pub(super) const RAMPS: &[NamedRamp] = &[
    // Blue through cyan, green, yellow and orange to red: the sweep that reads
    // as "start to end" along a chain.
    ramp(
        "rainbow",
        &[
            0x00_00_ff, 0x00_ff_ff, 0x00_ff_00, 0xff_ff_00, 0xff_80_00, 0xff_00_00,
        ],
    ),
    // The improved rainbow of Google's turbo, resampled evenly at sixteen
    // stops from the reference's fifty-one. The sweep is reproduced rather than
    // invented: a hand-picked set put a visibly different hue on the same
    // residue, and the ramp type holds sixteen stops.
    ramp(
        "turbo",
        &[
            0x23_17_1b, 0x42_2f_85, 0x4a_58_dd, 0x3b_82_f8, 0x2a_aa_ef, 0x25_cc_d0, 0x31_e8_aa,
            0x4d_f8_84, 0x76_fe_63, 0xa4_f7_49, 0xd1_e4_36, 0xea_d3_2e, 0xff_b1_25, 0xff_89_1e,
            0xf6_5f_18, 0xad_19_05,
        ],
    ),
    ramp(
        "rainbow_cycle",
        &[
            0xff_00_ff, 0x00_00_ff, 0x00_ff_ff, 0x00_ff_00, 0xff_ff_00, 0xff_80_00, 0xff_00_00,
            0xff_00_ff,
        ],
    ),
    ramp(
        "gcbmry",
        &[
            0x00_ff_00, 0x00_ff_ff, 0x00_00_ff, 0xff_00_ff, 0xff_00_00, 0xff_ff_00,
        ],
    ),
    ramp("cbmr", &[0x00_ff_ff, 0x00_00_ff, 0xff_00_ff, 0xff_00_00]),
    ramp("green_yellow_red", &[0x00_ff_00, 0xff_ff_00, 0xff_00_00]),
    ramp(
        "rainbow2",
        &[0x00_00_ff, 0x00_ff_ff, 0x00_ff_00, 0xff_ff_00, 0xff_00_00],
    ),
];
