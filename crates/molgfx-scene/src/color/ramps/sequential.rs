//! Sequential ramps: low to high in one direction.

use super::{NamedRamp, RampKind};

const fn ramp(name: &'static str, anchors: &'static [u32]) -> NamedRamp {
    NamedRamp {
        name,
        kind: RampKind::Sequential,
        anchors,
    }
}

pub(super) const RAMPS: &[NamedRamp] = &[
    ramp(
        "viridis",
        &[
            0x44_01_54, 0x48_28_78, 0x3e_49_89, 0x31_68_8e, 0x26_82_8e, 0x1f_9e_89, 0x35_b7_79,
            0x6e_ce_58, 0xb5_de_2b, 0xfd_e7_25,
        ],
    ),
    ramp(
        "plasma",
        &[
            0x0d_08_87, 0x46_03_9f, 0x72_01_a8, 0x9c_17_9e, 0xbd_37_86, 0xd8_57_6b, 0xed_79_53,
            0xfb_9f_3a, 0xfd_ca_26, 0xf0_f9_21,
        ],
    ),
    ramp(
        "magma",
        &[
            0x00_00_04, 0x18_0f_3d, 0x44_0f_76, 0x72_1f_81, 0x9e_2f_7f, 0xcd_40_71, 0xf1_60_5d,
            0xfd_95_67, 0xfe_c9_8d, 0xfc_fd_bf,
        ],
    ),
    ramp(
        "inferno",
        &[
            0x00_00_04, 0x1b_0c_41, 0x4a_0c_6b, 0x78_1c_6d, 0xa5_2c_60, 0xcf_44_46, 0xed_69_25,
            0xfb_9b_06, 0xf7_d1_3d, 0xfc_ff_a4,
        ],
    ),
    ramp(
        "cividis",
        &[
            0x00_22_4e, 0x12_35_70, 0x3b_49_6c, 0x57_5d_6d, 0x70_71_73, 0x8a_86_78, 0xa5_9c_74,
            0xc3_b3_69, 0xe1_cc_55, 0xfe_e8_38,
        ],
    ),
    ramp(
        "purples",
        &[
            0xfc_fb_fd, 0xef_ed_f5, 0xda_da_eb, 0xbc_bd_dc, 0x9e_9a_c8, 0x80_7d_ba, 0x6a_51_a3,
            0x54_27_8f, 0x3f_00_7d,
        ],
    ),
    ramp(
        "blues",
        &[
            0xf7_fb_ff, 0xde_eb_f7, 0xc6_db_ef, 0x9e_ca_e1, 0x6b_ae_d6, 0x42_92_c6, 0x21_71_b5,
            0x08_51_9c, 0x08_30_6b,
        ],
    ),
    ramp(
        "greens",
        &[
            0xf7_fc_f5, 0xe5_f5_e0, 0xc7_e9_c0, 0xa1_d9_9b, 0x74_c4_76, 0x41_ab_5d, 0x23_8b_45,
            0x00_6d_2c, 0x00_44_1b,
        ],
    ),
    ramp(
        "oranges",
        &[
            0xff_f5_eb, 0xfe_e6_ce, 0xfd_d0_a2, 0xfd_ae_6b, 0xfd_8d_3c, 0xf1_69_13, 0xd9_48_01,
            0xa6_36_03, 0x7f_27_04,
        ],
    ),
    ramp(
        "reds",
        &[
            0xff_f5_f0, 0xfe_e0_d2, 0xfc_bb_a1, 0xfc_92_72, 0xfb_6a_4a, 0xef_3b_2c, 0xcb_18_1d,
            0xa5_0f_15, 0x67_00_0d,
        ],
    ),
    ramp(
        "greys",
        &[
            0xff_ff_ff, 0xf0_f0_f0, 0xd9_d9_d9, 0xbd_bd_bd, 0x96_96_96, 0x73_73_73, 0x52_52_52,
            0x25_25_25, 0x00_00_00,
        ],
    ),
    ramp(
        "hot",
        &[
            0x0b_00_00, 0x80_00_00, 0xff_00_00, 0xff_80_00, 0xff_ff_00, 0xff_ff_ff,
        ],
    ),
    ramp(
        "ocean",
        &[0x00_80_00, 0x00_40_80, 0x00_00_ff, 0x00_80_ff, 0xff_ff_ff],
    ),
    ramp(
        "afmhot",
        &[0x00_00_00, 0x80_00_00, 0xff_80_00, 0xff_ff_80, 0xff_ff_ff],
    ),
    ramp("grayscale", &[0x00_00_00, 0xff_ff_ff]),
];
