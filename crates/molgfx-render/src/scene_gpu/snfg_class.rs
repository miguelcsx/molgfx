//! Lowering of SNFG symbol shapes onto the analytic primitive families.
//!
//! A symbol never owns geometry: each shape maps to one family and one shape
//! code, and the existing instanced pipelines intersect it per pixel. Circles
//! reuse the particle sphere, squares and bars the oriented box, and the
//! polygonal symbols the regular-polygon prism, so a glycan scene of any size
//! is a handful of instanced draws with no vertex data behind them.

use super::primitive_draw::{FAMILY_BOX, FAMILY_PARTICLE, FAMILY_POLYGON};
use molgfx_core::CarbohydrateShape;
use molgfx_math::Rgba8;

/// Polygon shape codes the shader intersects beyond the legacy classes.
pub(crate) const POLYGON_REGULAR_HEXAGON: u32 = 1;
pub(crate) const POLYGON_REGULAR_PENTAGON: u32 = 5;
pub(crate) const POLYGON_TRIANGLE: u32 = 7;
pub(crate) const POLYGON_DIAMOND: u32 = 8;
pub(crate) const POLYGON_STAR: u32 = 9;

/// The particle shape value of a sphere.
const PARTICLE_SPHERE: u32 = 0;

/// Returns the primitive family and shape code a carbohydrate shape draws as.
///
/// The legacy classes keep their historical polygon code so existing records
/// render exactly as before.
pub(crate) const fn carbohydrate_class(shape: CarbohydrateShape) -> (u32, u32) {
    match shape {
        CarbohydrateShape::Sphere => (FAMILY_PARTICLE, PARTICLE_SPHERE),
        CarbohydrateShape::Cube | CarbohydrateShape::Bar => (FAMILY_BOX, 0),
        CarbohydrateShape::Diamond => (FAMILY_POLYGON, POLYGON_DIAMOND),
        CarbohydrateShape::Triangle => (FAMILY_POLYGON, POLYGON_TRIANGLE),
        CarbohydrateShape::Star => (FAMILY_POLYGON, POLYGON_STAR),
        CarbohydrateShape::Hexagon => (FAMILY_POLYGON, POLYGON_REGULAR_HEXAGON),
        CarbohydrateShape::Pentagon => (FAMILY_POLYGON, POLYGON_REGULAR_PENTAGON),
        other => (FAMILY_POLYGON, other.stable_code()),
    }
}

/// Split modes the primitive shader fills with the secondary colour.
const SPLIT_DIAGONAL: f32 = 1.0;
const SPLIT_HORIZONTAL: f32 = 2.0;
const SPLIT_VERTICAL: f32 = 3.0;

/// The secondary colour and split mode of a symbol, `[r, g, b, mode]`.
///
/// In the symbol frame (y toward the anomeric carbon) SNFG colours the upper
/// right half of a hexosamine square, divided along the diagonal from its
/// upper left to its lower right corner; the upper half of a uronic acid
/// diamond; and the right half of an N-acetyl deoxyhexosamine triangle. The
/// secondary colour fills the opposite half. Any other shape stays one
/// colour.
pub(crate) fn secondary_split(shape: CarbohydrateShape, secondary: Option<Rgba8>) -> [f32; 4] {
    let mode = match shape {
        CarbohydrateShape::Cube => SPLIT_DIAGONAL,
        CarbohydrateShape::Diamond => SPLIT_HORIZONTAL,
        CarbohydrateShape::Triangle => SPLIT_VERTICAL,
        _ => return [0.0; 4],
    };
    match secondary {
        Some(color) => {
            let [red, green, blue, _] = color.to_f32();
            [red, green, blue, mode]
        }
        None => [0.0; 4],
    }
}

#[cfg(test)]
#[path = "snfg_class_tests.rs"]
mod tests;
