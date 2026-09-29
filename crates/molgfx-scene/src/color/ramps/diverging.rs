//! Diverging ramps: two directions away from a neutral middle.

use super::{NamedRamp, RampKind};

const fn ramp(name: &'static str, anchors: &'static [u32]) -> NamedRamp {
    NamedRamp {
        name,
        kind: RampKind::Diverging,
        anchors,
    }
}

pub(super) const RAMPS: &[NamedRamp] = &[
    ramp(
        "coolwarm",
        &[
            0x3b_4c_c0, 0x67_88_ee, 0x9a_bb_ff, 0xc9_d7_f0, 0xed_d1_c2, 0xf7_a8_89, 0xe2_69_52,
            0xb4_04_26,
        ],
    ),
    ramp("red_white_blue", &[0xbf_22_22, 0xff_ff_ff, 0x33_61_e1]),
    ramp("blue_white_red", &[0x33_61_e1, 0xff_ff_ff, 0xbf_22_22]),
    ramp("blue_white_green", &[0x33_61_e1, 0xff_ff_ff, 0x1a_9a_41]),
    ramp("green_white_magenta", &[0x1a_9a_41, 0xff_ff_ff, 0xd0_1c_8b]),
    ramp("red_white_green", &[0xbf_22_22, 0xff_ff_ff, 0x1a_9a_41]),
    ramp("cyan_white_magenta", &[0x00_ff_ff, 0xff_ff_ff, 0xff_00_ff]),
    ramp(
        "red_yellow_green",
        &[
            0xa5_00_26, 0xd7_30_27, 0xf4_6d_43, 0xfd_ae_61, 0xfe_e0_8b, 0xff_ff_bf, 0xd9_ef_8b,
            0xa6_d9_6a, 0x66_bd_63, 0x1a_98_50, 0x00_68_37,
        ],
    ),
    ramp(
        "red_blue",
        &[
            0x67_00_1f, 0xb2_18_2b, 0xd6_60_4d, 0xf4_a5_82, 0xfd_db_c7, 0xf7_f7_f7, 0xd1_e5_f0,
            0x92_c5_de, 0x43_93_c3, 0x21_66_ac, 0x05_30_61,
        ],
    ),
    ramp(
        "spectral",
        &[
            0x9e_01_42, 0xd5_3e_4f, 0xf4_6d_43, 0xfd_ae_61, 0xfe_e0_8b, 0xff_ff_bf, 0xe6_f5_98,
            0xab_dd_a4, 0x66_c2_a5, 0x32_88_bd, 0x5e_4f_a2,
        ],
    ),
];
