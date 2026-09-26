//! Human-readable explanations of what a session's names mean.

use super::Session;
use super::resolve::Resolver;
use crate::error::{CommandError, ErrorKind};
use crate::ir::Name;
use std::fmt::Write as _;

impl Session {
    /// What a named selection is declared as, what it resolves to, and what
    /// depends on it.
    ///
    /// # Errors
    ///
    /// Returns an unknown-symbol error for a name that is not a selection.
    pub fn explain_selection(&self, name: &str) -> Result<String, CommandError> {
        let resolver = Resolver {
            state: &self.state,
            site: None,
        };
        let Some(query) = self.state.selection(name) else {
            return Err(resolver.unknown_selection(name));
        };
        let meaning = resolver.selection(query)?;
        let mut text = format!(
            "selection {name}\n  declared: {query}\n  resolved: {}",
            meaning.source()
        );
        let _ = write!(text, "\n  fingerprint: {}", query.fingerprint());
        let users = self.state.users_of(name);
        if !users.is_empty() {
            let _ = write!(text, "\n  used by: {}", users.join(", "));
        }
        Ok(text)
    }

    /// What a layer draws and from which structure.
    ///
    /// # Errors
    ///
    /// Returns an unknown-symbol error for a name that is not a layer.
    pub fn explain_layer(&self, name: &str) -> Result<String, CommandError> {
        let resolver = Resolver {
            state: &self.state,
            site: None,
        };
        let layer = Name::new(name)
            .ok()
            .and_then(|name| self.state.spec.layers.get(&name))
            .ok_or_else(|| resolver.unknown_layer(name))?;
        let meaning = resolver.selection(&layer.target)?;
        let structure = self
            .state
            .structure_name(layer.structure)
            .map_or_else(|| layer.structure.get().to_string(), ToString::to_string);
        Ok(format!(
            "layer @{name}\n  form: {}\n  structure: {structure}\n  declared: {}\n  resolved: {}\n  representation: {}",
            layer.form.kind().name(),
            layer.target,
            meaning.source(),
            layer.id.get(),
        ))
    }

    /// Everything the session knows, one line per name.
    #[must_use]
    pub fn summary(&self) -> String {
        let spec = &self.state.spec;
        let mut lines = Vec::new();
        for (name, id) in &spec.structures {
            lines.push(format!("structure {name} = {}", id.get()));
        }
        for (name, query) in &spec.selections {
            lines.push(format!("select {name}, {query}"));
        }
        for (name, layer) in &spec.layers {
            lines.push(format!(
                "layer @{name}: {} of {}",
                layer.form.kind().name(),
                layer.target
            ));
        }
        for rule in spec.rules.values() {
            lines.push(format!("color {}, {}", rule.color, rule.target));
        }
        if let Some(focus) = &spec.focus {
            lines.push(format!("focus {focus}"));
        }
        lines.join("\n")
    }

    /// Resolves a structure name, for bindings that address one by name.
    ///
    /// # Errors
    ///
    /// Returns an unknown-symbol error.
    pub fn structure(&self, name: &str) -> Result<molgfx_api::StructureId, CommandError> {
        Name::new(name)
            .ok()
            .and_then(|name| self.state.spec.structures.get(&name).copied())
            .ok_or_else(|| {
                CommandError::new(
                    ErrorKind::UnknownSymbol,
                    format!("there is no structure named '{name}'"),
                )
            })
    }
}
