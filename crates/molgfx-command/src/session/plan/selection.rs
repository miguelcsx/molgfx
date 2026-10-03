//! Current selection edits participate in the Session's atomic history.

use super::{Planner, scene_error};
use crate::error::{CommandError, ErrorKind};
use crate::ir::QueryText;
use molgfx_scene::{InteractionChannel, PatchOperation};

impl Planner<'_> {
    pub(super) fn set_selection(&mut self, query: Option<&QueryText>) -> Result<(), CommandError> {
        let selection = query
            .map(|query| self.resolver().selection(query))
            .transpose()?;
        match &selection {
            Some(selection) => {
                // Expand references before replacing $sel, so selecting a subset
                // of the previous selection never creates a self-referential alias.
                let resolved = selection.compiled().map_err(|error| scene_error(&error))?;
                self.state
                    .aliases
                    .define("sel", (*resolved).clone())
                    .map_err(|diagnostics| {
                        CommandError::new(ErrorKind::Query, diagnostics.to_string())
                    })?;
            }
            None => {
                let _ = self.state.aliases.remove("sel");
            }
        }
        self.transaction
            .stage(PatchOperation::SetInteraction {
                channel: InteractionChannel::Selected,
                selection,
            })
            .map_err(|error| scene_error(&error))
    }
}
