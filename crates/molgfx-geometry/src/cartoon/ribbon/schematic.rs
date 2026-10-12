//! Resident-coordinate stencils for schematic strands, in O(guides + vertices).

use super::{RibbonDeformation, control_rows};
use molgfx_core::SecondaryStructure;
use std::collections::HashMap;

pub(super) fn prepare(
    recipes: &mut [RibbonDeformation],
    entities: &[u32],
    styles: &[SecondaryStructure],
    plane_rows: &[u32],
    properties: &mut Vec<f32>,
    filter_strands: bool,
) -> Result<(), crate::PackingError> {
    let mut stencils = HashMap::with_capacity(entities.len());
    for index in 0..entities.len() {
        let rows = control_rows(entities, index);
        if rows[1] == u32::MAX {
            continue;
        }
        let offset = super::super::draw_limits::vertex_index(properties.len())?;
        let oxygen = match plane_rows.get(index) {
            Some(&row) => row,
            None => u32::MAX,
        };
        let filter =
            u32::from(filter_strands && styles.get(index).is_some_and(|style| style.is_strand()));
        properties.extend([rows[0], rows[1], rows[2], filter, oxygen].map(f32::from_bits));
        stencils.insert(rows[1], offset);
    }
    for recipe in recipes {
        if !matches!(recipe.parameter[3].to_bits(), 0 | 4) {
            continue;
        }
        let mapped = recipe.controls.map(|row| stencils.get(&row).copied());
        let [Some(a), Some(b), Some(c), Some(d)] = mapped else {
            continue;
        };
        recipe.controls = [a, b, c, d];
        recipe.parameter[3] = f32::from_bits(14);
    }
    Ok(())
}

#[cfg(test)]
#[path = "schematic_tests.rs"]
mod tests;
