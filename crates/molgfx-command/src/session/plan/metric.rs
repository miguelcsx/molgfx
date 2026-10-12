//! A continuous colour named in a `color` rule.
//!
//! A metric colour maps a value through a ramp whose domain and legend
//! describe one representation, so it cannot colour part of a layer the way a
//! flat colour rule does. With no target, though, the author means the whole
//! structure, and that is exactly every layer drawn from it.

use super::Planner;
use crate::error::{CommandError, ErrorKind};
use crate::ir::{ColorValue, Name, QueryText};
use molgfx_scene::StructureId;

impl Planner<'_> {
    /// Colours every layer of `structure` by `color` when the target is the
    /// whole structure, and otherwise explains the form that works.
    pub(super) fn metric_color(
        &mut self,
        color: &ColorValue,
        query: &QueryText,
        structure: StructureId,
    ) -> Result<(), CommandError> {
        let layers: Vec<(Name, _)> = self
            .state
            .spec
            .layers
            .iter()
            .filter(|(_, layer)| layer.structure == structure)
            .map(|(name, layer)| (name.clone(), layer.id))
            .collect();
        if query.source() == "all" && !layers.is_empty() {
            let spec = self.color_spec(color, structure)?;
            for (_, id) in layers {
                self.transaction
                    .set_color(id, spec.clone())
                    .map_err(|error| super::scene_error(&error))?;
            }
            return Ok(());
        }
        let names: Vec<String> = layers.iter().map(|(name, _)| format!("@{name}")).collect();
        let hint = match names.first() {
            Some(first) => format!(
                "write color {color}, {first} to colour a whole layer (layers: {}), color {color} for every layer, or show FORM color={color}, {query} to draw the selection with it",
                names.join(", ")
            ),
            None => format!(
                "draw it first with show FORM color={color}, {query}, then recolour that layer with color {color}, @LAYER"
            ),
        };
        Err(CommandError::new(
            ErrorKind::InvalidColor,
            format!(
                "'{color}' is a continuous colour, so it colours whole layers, not part of one ({query}); {hint}"
            ),
        ))
    }
}
