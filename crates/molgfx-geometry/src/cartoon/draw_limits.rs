//! Checks native draw limits before a sweep emits any geometry.

use crate::PackingError;

pub(super) fn vertex_index(index: usize) -> Result<u32, PackingError> {
    checked_u32("ribbon vertices", index as u64)
}

pub(super) fn check_sweep(
    vertex_offset: usize,
    index_offset: usize,
    rings: usize,
    shell_indices: u64,
    flat_caps: bool,
) -> Result<(), PackingError> {
    let cap_vertices = if flat_caps { 8 } else { 34 };
    let cap_indices = if flat_caps { 12 } else { 96 };
    let vertices = (rings as u64)
        .checked_mul(super::ribbon::PROFILE_SIDES as u64)
        .and_then(|count| count.checked_add(cap_vertices))
        .and_then(|count| count.checked_add(vertex_offset as u64))
        .ok_or(PackingError::IndexOverflow {
            resource: "ribbon vertices",
            index: u64::MAX,
        })?;
    checked_u32("ribbon vertices", vertices)?;
    let indices = shell_indices
        .checked_add(cap_indices)
        .and_then(|count| count.checked_add(index_offset as u64))
        .ok_or(PackingError::IndexOverflow {
            resource: "ribbon draw indices",
            index: u64::MAX,
        })?;
    checked_u32("ribbon draw indices", indices)?;
    Ok(())
}

fn checked_u32(resource: &'static str, index: u64) -> Result<u32, PackingError> {
    u32::try_from(index).map_err(|_| PackingError::IndexOverflow { resource, index })
}

pub(super) fn check_glyph(vertices: usize, indices: usize) -> Result<(), PackingError> {
    checked_u32("ribbon vertices", (vertices as u64).saturating_add(3))?;
    checked_u32("ribbon draw indices", (indices as u64).saturating_add(3))?;
    Ok(())
}

/// Checks the complete dash stream before reserving or emitting any vertices.
pub(super) fn check_gap_capacity(
    vertices: usize,
    indices: usize,
    length: f32,
    period: f32,
) -> Result<(), PackingError> {
    let dashes = (f64::from(length) / f64::from(period)).ceil();
    let sides = checked_u32("ribbon profile sides", super::ribbon::PROFILE_SIDES as u64)?;
    // Two side rings and two independently shaded, closed end fans.
    let dash_vertices = f64::from(4 * sides + 2);
    let dash_indices = f64::from(12 * sides);
    for (resource, offset, count) in [
        ("ribbon vertices", vertices, dash_vertices),
        ("ribbon draw indices", indices, dash_indices),
    ] {
        let offset = checked_u32(resource, offset as u64)?;
        if f64::from(offset) + dashes * count > f64::from(u32::MAX) {
            return Err(PackingError::IndexOverflow {
                resource,
                index: u64::from(u32::MAX) + 1,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "draw_limits_tests.rs"]
mod tests;
