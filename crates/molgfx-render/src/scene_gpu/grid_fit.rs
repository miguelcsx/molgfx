//! Fitting one sampled surface field inside its size limits.

use molgfx_math::Vec3;

/// A field's cell size and dimensions, and whether the memory budget set them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct GridFit {
    pub(super) cell: f32,
    pub(super) dimensions: [u32; 3],
    /// The field was coarsened to fit `max_cells`, not by an axis or the device.
    pub(super) memory_limited: bool,
}

/// How much one coarsening step grows the cell; about 12 % fewer voxels each.
const COARSENING_STEP: f32 = 1.04;

/// Fits a field spanning `extent` ångström inside `limits`.
///
/// The cell is the requested spacing unless an axis would pass the longest
/// allowed, and then it grows in small steps until the whole field fits the
/// voxel budget. The result is what shading, generation and the reported
/// spacing all read, so none of them can disagree about it.
pub(super) fn fit_grid(extent: Vec3, limits: super::detail::GridLimits) -> GridFit {
    let max_divisions = dimension_f32(limits.max_dimension.saturating_sub(1));
    let mut cell = (extent.max_element() / max_divisions).max(limits.spacing);
    let mut memory_limited = false;
    loop {
        let dimensions = [
            axis_cells(extent.x, cell, limits.max_dimension),
            axis_cells(extent.y, cell, limits.max_dimension),
            axis_cells(extent.z, cell, limits.max_dimension),
        ];
        let voxels: u64 = dimensions.iter().map(|&axis| u64::from(axis)).product();
        if voxels <= u64::from(limits.max_cells) || !cell.is_finite() {
            return GridFit {
                cell,
                dimensions,
                memory_limited,
            };
        }
        memory_limited = true;
        cell *= COARSENING_STEP;
    }
}

fn axis_cells(extent: f32, cell: f32, max_dimension: u32) -> u32 {
    let mut cells = 2u32;
    while cells < max_dimension && dimension_f32(cells.saturating_sub(1)) * cell < extent {
        cells += 1;
    }
    cells
}

fn dimension_f32(value: u32) -> f32 {
    f32::from(crate::fallback(u16::try_from(value), u16::MAX))
}

#[cfg(test)]
#[path = "grid_fit_tests.rs"]
mod tests;
