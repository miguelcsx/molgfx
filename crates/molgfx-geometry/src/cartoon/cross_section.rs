//! Fixed-size cross-section samples and their geometric face normals.
//!
//! Every profile uses the same sixteen lanes, preserving ribbon deformation
//! storage and the shared draw pipeline. Sampling costs O(1) per vertex.

use super::ribbon::PROFILE_SIDES;
use molgfx_core::CartoonProfile;

const ELLIPSE: [[f32; 2]; PROFILE_SIDES] = [
    [1.0, 0.0],
    [0.923_879_5, 0.382_683_43],
    [0.707_106_77, 0.707_106_77],
    [0.382_683_43, 0.923_879_5],
    [0.0, 1.0],
    [-0.382_683_43, 0.923_879_5],
    [-0.707_106_77, 0.707_106_77],
    [-0.923_879_5, 0.382_683_43],
    [-1.0, 0.0],
    [-0.923_879_5, -0.382_683_43],
    [-0.707_106_77, -0.707_106_77],
    [-0.382_683_43, -0.923_879_5],
    [0.0, -1.0],
    [0.382_683_43, -0.923_879_5],
    [0.707_106_77, -0.707_106_77],
    [0.923_879_5, -0.382_683_43],
];

const THIRD: f32 = 1.0 / 3.0;
const SQUARE: [[f32; 4]; PROFILE_SIDES] = [
    [1.0, 1.0, 0.0, 1.0],
    [THIRD, 1.0, 0.0, 1.0],
    [-THIRD, 1.0, 0.0, 1.0],
    [-1.0, 1.0, 0.0, 1.0],
    [-1.0, 1.0, -1.0, 0.0],
    [-1.0, THIRD, -1.0, 0.0],
    [-1.0, -THIRD, -1.0, 0.0],
    [-1.0, -1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0, -1.0],
    [-THIRD, -1.0, 0.0, -1.0],
    [THIRD, -1.0, 0.0, -1.0],
    [1.0, -1.0, 0.0, -1.0],
    [1.0, -1.0, 1.0, 0.0],
    [1.0, -THIRD, 1.0, 0.0],
    [1.0, THIRD, 1.0, 0.0],
    [1.0, 1.0, 1.0, 0.0],
];

const SIN_SIXTY: f32 = 0.866_025_4;
// Normal components and the signed semicircle-centre displacement. The middle
// of each planar face has displacement zero and shares its face normal.
const STADIUM: [[f32; 3]; PROFILE_SIDES] = [
    [1.0, 0.0, 1.0],
    [SIN_SIXTY, 0.5, 1.0],
    [0.5, SIN_SIXTY, 1.0],
    [0.0, 1.0, 1.0],
    [0.0, 1.0, 0.0],
    [0.0, 1.0, -1.0],
    [-0.5, SIN_SIXTY, -1.0],
    [-SIN_SIXTY, 0.5, -1.0],
    [-1.0, 0.0, -1.0],
    [-SIN_SIXTY, -0.5, -1.0],
    [-0.5, -SIN_SIXTY, -1.0],
    [0.0, -1.0, -1.0],
    [0.0, -1.0, 0.0],
    [0.0, -1.0, 1.0],
    [0.5, -SIN_SIXTY, 1.0],
    [SIN_SIXTY, -0.5, 1.0],
];

pub(super) const SQUARE_CORNERS: [[f32; 2]; 4] =
    [[1.0, 1.0], [-1.0, 1.0], [-1.0, -1.0], [1.0, -1.0]];

/// Offset coefficients and the corresponding physical normal coefficients.
#[inline]
pub(super) fn cross_section(
    profile: CartoonProfile,
    side: usize,
    width: f32,
    depth: f32,
) -> (f32, f32, [f32; 2]) {
    let side = side % PROFILE_SIDES;
    match profile {
        CartoonProfile::Square => {
            let [x, y, normal_x, normal_y] = SQUARE[side];
            (x, y, [normal_x, normal_y])
        }
        CartoonProfile::Rounded if width > 0.0 && depth > 0.0 => rounded(side, width, depth),
        CartoonProfile::Elliptical | CartoonProfile::Rounded => {
            let [x, y] = ELLIPSE[side];
            (x, y, [x * depth, y * width])
        }
    }
}

fn rounded(side: usize, width: f32, depth: f32) -> (f32, f32, [f32; 2]) {
    let [x, y, centre] = STADIUM[side];
    if width >= depth {
        ((x * depth + centre * (width - depth)) / width, y, [x, y])
    } else {
        (-y, (x * width + centre * (depth - width)) / depth, [-y, x])
    }
}

#[cfg(test)]
#[path = "cross_section_tests.rs"]
mod tests;
