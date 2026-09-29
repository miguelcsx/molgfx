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
    ramp(
        "turbo",
        &[
            0x30_12_3b, 0x41_45_ab, 0x46_75_ed, 0x39_a2_fc, 0x1b_cf_d4, 0x24_ec_a6, 0x61_fc_6c,
            0xa4_fc_3b, 0xd1_e8_34, 0xf3_c6_3a, 0xfe_9b_2d, 0xf3_63_15, 0xd9_38_06, 0xb1_19_01,
            0x7a_04_02,
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
