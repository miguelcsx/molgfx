//! SNFG (Symbol Nomenclature for Glycans) symbols for the carbohydrates of a
//! structure, as instance records for the analytic primitive pipelines.
//!
//! Identity comes from upstream, never from here: molframe resolves each
//! complete saccharide ring to a curated SNFG symbol from its CCD component,
//! and this module only lowers that symbol to a renderable glyph, a pose and a
//! colour. One build pass is linear in the number of monosaccharides and
//! produces column arrays; every later use is a copy of those columns, so a
//! frame never walks glycans. The glyphs themselves are intersected per pixel
//! by the instanced primitive pipelines, which keeps a scene of thousands of
//! glycans at a few draws with no vertex data.
//!
//! Shapes follow the SNFG specification (Varki et al., Glycobiology 2015,
//! 25:1323): circles for hexoses, squares for N-acetylhexosamines, diamonds
//! for acidic sugars, triangles for deoxyhexoses and stars for pentoses.
//! Crossed and divided symbols (hexosamines, uronic acids, N-acetyl
//! deoxyhexosamines) carry their secondary colour in the set but draw in the
//! primary colour only.

use crate::PackingError;
use molframe::chemistry::SnfgShape;
use molgfx_core::{
    AtomSelection, CarbohydrateShape, CarbohydrateSymbol, Primitive, StructureHandle,
};
use molgfx_math::{Mat3, Quat, Rgba8, Vec3};

#[cfg(test)]
#[path = "snfg_tests.rs"]
mod tests;

/// Edge or diameter of a symbol in ångström at scale one: a little smaller
/// than a pyranose ring so neighbouring symbols stay separate.
pub const SNFG_SYMBOL_SIZE: f32 = 2.2;

/// Lowers an upstream SNFG shape to the renderable glyph.
///
/// Generic ring prisms upstream use for unmapped sugars become hexagons, the
/// SNFG symbol for an unclassified monosaccharide.
#[must_use]
pub const fn glyph(shape: SnfgShape) -> CarbohydrateShape {
    match shape {
        SnfgShape::FilledSphere => CarbohydrateShape::Sphere,
        SnfgShape::FilledCube | SnfgShape::CrossedCube => CarbohydrateShape::Cube,
        SnfgShape::DividedDiamond
        | SnfgShape::FilledDiamond
        | SnfgShape::FlatDiamond
        | SnfgShape::DiamondPrism => CarbohydrateShape::Diamond,
        SnfgShape::FilledCone | SnfgShape::DividedCone => CarbohydrateShape::Triangle,
        SnfgShape::FlatBox => CarbohydrateShape::Bar,
        SnfgShape::FilledStar => CarbohydrateShape::Star,
        SnfgShape::Pentagon | SnfgShape::PentagonalPrism => CarbohydrateShape::Pentagon,
        SnfgShape::FlatHexagon | SnfgShape::HexagonalPrism | SnfgShape::HeptagonalPrism => {
            CarbohydrateShape::Hexagon
        }
    }
}

/// Full extents of a glyph at scale one, in the symbol's local frame whose
/// z axis is the ring normal.
#[must_use]
pub fn glyph_size(glyph: CarbohydrateShape) -> Vec3 {
    let unit = SNFG_SYMBOL_SIZE;
    match glyph {
        CarbohydrateShape::Sphere => Vec3::splat(unit),
        CarbohydrateShape::Cube => Vec3::splat(unit * 0.82),
        CarbohydrateShape::Bar => Vec3::new(unit, unit * 0.45, unit * 0.45),
        CarbohydrateShape::Star => Vec3::new(unit * 1.4, unit * 1.4, unit * 0.3),
        CarbohydrateShape::Triangle => Vec3::new(unit * 1.1, unit * 1.1, unit * 0.3),
        _ => Vec3::new(unit, unit, unit * 0.3),
    }
}

fn pack_rgb(rgb: u32) -> Rgba8 {
    let [_, red, green, blue] = rgb.to_be_bytes();
    Rgba8::opaque(red, green, blue)
}

/// Orients a symbol with its thin axis along the ring normal and its up axis
/// toward the anomeric carbon, so a branch reads the same way for every
/// residue; falls back to any perpendicular when the direction is unknown.
fn pose(normal: [f32; 3], anomeric: Option<[f32; 3]>) -> Option<Quat> {
    let z = Vec3::from(normal).try_normalize()?;
    let up = anomeric
        .map(Vec3::from)
        .and_then(|direction| (direction - z * z.dot(direction)).try_normalize())
        .or_else(|| z.any_orthonormal_vector().into())?;
    let x = up.cross(z).try_normalize()?;
    let y = z.cross(x);
    let quat = Quat::from_mat3(&Mat3::from_cols(x, y, z));
    quat.is_finite().then_some(quat)
}

/// Glyph, primary colour (0xRRGGBB) and community abbreviation of a CCD
/// component, or `None` when it is not a curated monosaccharide.
#[must_use]
pub fn component_symbol(component: &str) -> Option<(CarbohydrateShape, u32, &'static str)> {
    molframe::chemistry::snfg_symbol(component)
        .map(|symbol| (glyph(symbol.shape), symbol.color, symbol.abbreviation))
}

/// Column storage of every SNFG symbol of a selection, built once.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SnfgSymbolSet {
    /// Ring centroids in model space.
    pub centers: Vec<Vec3>,
    /// Symbol orientations.
    pub orientations: Vec<Quat>,
    /// Glyph of each symbol.
    pub shapes: Vec<CarbohydrateShape>,
    /// Primary SNFG colour of each symbol.
    pub colors: Vec<Rgba8>,
    /// Secondary colour of crossed and divided symbols.
    pub secondary: Vec<Option<Rgba8>>,
    /// Source residue ordinal of each symbol.
    pub residues: Vec<u32>,
    /// Start of each symbol's ring atoms in `ring_atoms`, plus a final end.
    ring_offsets: Vec<u32>,
    /// Ring atom rows of every symbol, concatenated.
    ring_atoms: Vec<u32>,
}

impl SnfgSymbolSet {
    /// Builds the symbols of every complete selected ring.
    ///
    /// A ring with an incomplete selection or no resolvable plane produces no
    /// symbol: nothing is placed from a guessed position.
    ///
    /// # Errors
    /// Returns a topology error when molecular chemistry cannot resolve the
    /// source.
    pub fn build(
        structure: &molframe::Structure,
        selection: &AtomSelection,
    ) -> Result<Self, PackingError> {
        let report = molframe::chemistry::carbohydrates(
            structure.engine(),
            None,
            molframe::chemistry::CarbohydrateOptions::default(),
        )
        .map_err(|_| PackingError::Topology {
            operation: "saccharide symbol inventory",
        })?;
        let mut set = Self::default();
        set.ring_offsets.push(0);
        let count = report.monosaccharides.len();
        set.centers.reserve(count);
        set.orientations.reserve(count);
        set.shapes.reserve(count);
        set.colors.reserve(count);
        set.secondary.reserve(count);
        set.residues.reserve(count);
        for sugar in &report.monosaccharides {
            if !sugar
                .ring_atoms
                .iter()
                .all(|atom| selection.contains(atom.get()))
            {
                continue;
            }
            let Some(geometry) = sugar.geometry else {
                continue;
            };
            let Some(orientation) = pose(geometry.normal, geometry.anomeric_direction) else {
                continue;
            };
            let center = Vec3::from(geometry.center);
            if !center.is_finite() {
                continue;
            }
            set.centers.push(center);
            set.orientations.push(orientation);
            set.shapes.push(glyph(sugar.symbol.shape));
            set.colors.push(pack_rgb(sugar.symbol.color));
            set.secondary
                .push(sugar.symbol.secondary_color.map(pack_rgb));
            set.residues.push(sugar.residue.get());
            set.ring_atoms
                .extend(sugar.ring_atoms.iter().map(|atom| atom.get()));
            set.ring_offsets.push(offset_of(set.ring_atoms.len()));
        }
        Ok(set)
    }

    /// The symbols whose ring atoms are all selected, in the same order.
    ///
    /// Linear in the ring atoms of the set and free of any chemistry, so a set
    /// built once for a structure serves every selection over it.
    #[must_use]
    pub fn subset(&self, selection: &AtomSelection) -> Self {
        let mut out = Self::default();
        out.ring_offsets.push(0);
        for index in 0..self.len() {
            let start = self.ring_offsets[index] as usize;
            let end = self.ring_offsets[index + 1] as usize;
            let ring = &self.ring_atoms[start..end];
            if !ring.iter().all(|atom| selection.contains(*atom)) {
                continue;
            }
            out.centers.push(self.centers[index]);
            out.orientations.push(self.orientations[index]);
            out.shapes.push(self.shapes[index]);
            out.colors.push(self.colors[index]);
            out.secondary.push(self.secondary[index]);
            out.residues.push(self.residues[index]);
            out.ring_atoms.extend_from_slice(ring);
            out.ring_offsets.push(offset_of(out.ring_atoms.len()));
        }
        out
    }

    /// Number of symbols.
    #[must_use]
    pub fn len(&self) -> usize {
        self.centers.len()
    }

    /// Whether the set holds no symbol.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.centers.is_empty()
    }

    /// Lowers the columns to scene primitives owned by `owner`, at `scale`
    /// times the standard symbol size. Symbols whose record fails validation
    /// (a non-positive scale) are dropped.
    #[must_use]
    pub fn to_primitives(&self, owner: StructureHandle, scale: f32) -> Vec<Primitive> {
        let mut primitives = Vec::with_capacity(self.len());
        for index in 0..self.len() {
            let shape = self.shapes[index];
            let record = CarbohydrateSymbol::new(
                owner,
                self.centers[index],
                self.orientations[index],
                glyph_size(shape) * scale,
                shape,
                self.colors[index],
            );
            if let Ok(record) = record {
                primitives.push(Primitive::Carbohydrate(record));
            }
        }
        primitives
    }
}

/// A ring-atom offset as the packed `u32`, saturating past the addressable range.
fn offset_of(len: usize) -> u32 {
    let Ok(offset) = u32::try_from(len) else {
        return u32::MAX;
    };
    offset
}
