//! Planning the `auto` command into its default layers.
//!
//! The policy lives in the scene layer, which owns the structure
//! classification; this file only turns the chosen forms into a scene edit and
//! registers each under its form's name.

use super::Planner;
use super::scene_error;
use crate::error::{CommandError, ErrorKind};
use crate::ir::{Form, FormKind, Name, Positive, QueryText};
use crate::session::state::LayerSpec;
use molgfx_scene::preset::PocketStyle;

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
        let added = self.register_forms(forms, structure)?;
        self.messages
            .push(format!("{added} default layer(s) added"));
        Ok(())
    }

    /// Adds each form and registers its layer under the form's name, so later
    /// statements can hide, recolour or remove it like any other.
    fn register_forms(
        &mut self,
        forms: Vec<molgfx_scene::RepresentationSpec>,
        structure: molgfx_scene::StructureId,
    ) -> Result<usize, CommandError> {
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
                    form: Form::new(match FormKind::from_name(&kind) {
                        Some(form) => form,
                        None => FormKind::Cartoon,
                    }),
                    target: declared,
                },
            );
            added += 1;
        }
        Ok(added)
    }

    /// Draws the pocket-and-pose composition around `target` and focuses it.
    pub(super) fn pocket(
        &mut self,
        target: &QueryText,
        near: Option<Positive>,
        mid: Option<Positive>,
        structure: Option<&Name>,
    ) -> Result<(), CommandError> {
        let structure = self.structure(structure)?;
        let mut style = PocketStyle::default();
        if let Some(near) = near {
            style.near = near.get();
        }
        if let Some(mid) = mid {
            style.mid = mid.get();
        }
        let focus = molgfx_scene::Selection::from(target.source());
        let forms = molgfx_scene::preset::pocket_representations(&focus, structure, style)
            .map_err(|error| scene_error(&error))?;
        let added = self.register_forms(forms, structure)?;
        let selection = self.resolver().selection(target)?;
        self.transaction.focus(selection);
        self.state.spec.focus = Some(target.clone());
        self.messages.push(format!("{added} pocket layer(s) added"));
        Ok(())
    }
}
