//! Planning the `auto` command into its default layers.
//!
//! The policy lives in the scene layer, which owns the structure
//! classification; this file only turns the chosen forms into a scene edit and
//! registers each under its form's name.

use super::Planner;
use super::scene_error;
use crate::error::{CommandError, ErrorKind};
use crate::ir::{Form, FormKind, Name, QueryText};
use crate::session::state::LayerSpec;

impl Planner<'_> {
    /// Draws a structure with the size- and chemistry-appropriate default
    /// forms.
    ///
    /// The policy lives in the scene layer, which owns the classification; the
    /// command names only the structure, so the same rule applies whether a
    /// caller opens a file in a viewer or writes the statement by hand. Each
    /// added layer is registered under its form's name, so later statements can
    /// hide, recolour or remove it like any other.
    pub(super) fn auto(&mut self, structure: Option<&Name>) -> Result<(), CommandError> {
        let structure = self.structure(structure)?;
        let Some(source) = self.sources.get(&structure) else {
            return Err(CommandError::new(
                ErrorKind::Scene,
                "the structure is not bound to this session's scene",
            ));
        };
        let forms = molgfx_scene::preset::auto_representations(source, structure)
            .map_err(|error| scene_error(&error))?;
        let mut added = 0_usize;
        for form in forms {
            let kind = form.form_name().to_owned();
            let declared = QueryText::compile(form.target().source()).map_err(|diagnostics| {
                CommandError::new(
                    ErrorKind::Scene,
                    diagnostics.first().map_or_else(
                        || "the preset target is not a valid query".to_owned(),
                        ToString::to_string,
                    ),
                )
            })?;
            let id = self
                .transaction
                .add(form)
                .map_err(|error| scene_error(&error))?;
            let name = self.layer_name(&kind);
            let _ = self.state.spec.layers.insert(
                name,
                LayerSpec {
                    id,
                    structure,
                    form: Form::new(
                        FormKind::from_name(&kind).map_or(FormKind::Cartoon, |kind| kind),
                    ),
                    target: declared,
                },
            );
            added += 1;
        }
        self.messages
            .push(format!("{added} default layer(s) added"));
        Ok(())
    }
}
