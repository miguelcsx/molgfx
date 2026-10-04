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

#[cfg(test)]
#[path = "draw_limits_tests.rs"]
mod tests;
