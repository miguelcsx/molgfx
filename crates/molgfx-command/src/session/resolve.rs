//! Resolving declared targets through named selections.

use super::state::State;
use crate::error::{CommandError, ErrorKind, Span};
use crate::ir::{Name, QueryText};
use crate::registry;

pub(crate) struct Resolver<'a> {
    pub(crate) state: &'a State,
    /// The statement being planned, to locate a name in its text.
    pub(crate) site: Option<(&'a str, Span)>,
}

impl Resolver<'_> {
    /// Fails for the first reference to a selection that does not exist.
    pub(crate) fn check_references(&self, query: &QueryText) -> Result<(), CommandError> {
        match query
            .references()
            .into_iter()
            .find(|reference| self.state.selection(reference).is_none())
        {
            Some(missing) => Err(self.unknown_selection(missing)),
            None => Ok(()),
        }
    }

    /// The scene selection a declared query currently means.
    pub(crate) fn selection(
        &self,
        query: &QueryText,
    ) -> Result<molgfx_api::Selection, CommandError> {
        self.check_references(query)?;
        self.state
            .aliases
            .resolve(query.query())
            .map(molgfx_api::Selection::from)
            .map_err(|diagnostics| {
                let message = diagnostics.first().map_or_else(
                    || "the query could not be resolved".to_owned(),
                    |first| first.message().to_owned(),
                );
                let mut error = CommandError::new(ErrorKind::Query, message);
                error.code = diagnostics.first().map(|first| first.code().to_string());
                error
            })
    }

    pub(crate) fn unknown_selection(&self, name: &str) -> CommandError {
        let located = self
            .locate(&format!("${name}"))
            .or_else(|| self.locate(name));
        let error = if self.is_layer(name) {
            CommandError::new(
                ErrorKind::WrongSymbolKind,
                format!("'{name}' is a layer, not a selection; write @{name}"),
            )
        } else {
            CommandError::new(
                ErrorKind::UnknownSymbol,
                format!("there is no selection named '{name}'"),
            )
            .suggest(registry::suggest(
                name,
                self.state.spec.selections.keys().map(Name::as_str),
            ))
        };
        match located {
            Some(span) => error.at(span),
            None => error,
        }
    }

    pub(crate) fn unknown_layer(&self, name: &str) -> CommandError {
        let located = self
            .locate(&format!("@{name}"))
            .or_else(|| self.locate(name));
        let error = if self.state.selection(name).is_some() {
            CommandError::new(
                ErrorKind::WrongSymbolKind,
                format!("'{name}' is a selection, not a layer; draw it with show FORM, ${name}"),
            )
        } else {
            CommandError::new(
                ErrorKind::UnknownSymbol,
                format!("there is no layer named '@{name}'"),
            )
            .suggest(registry::suggest(
                name,
                self.state.spec.layers.keys().map(Name::as_str),
            ))
        };
        match located {
            Some(span) => error.at(span),
            None => error,
        }
    }

    fn is_layer(&self, name: &str) -> bool {
        Name::new(name).is_ok_and(|name| self.state.spec.layers.contains_key(&name))
    }

    /// The first whole-word occurrence of `needle` in the statement.
    fn locate(&self, needle: &str) -> Option<Span> {
        let (source, span) = self.site?;
        let text = source.get(span.start..span.end)?;
        let word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
        let mut from = 0;
        while let Some(offset) = text.get(from..)?.find(needle) {
            let start = from + offset;
            let end = start + needle.len();
            let before = start
                .checked_sub(1)
                .and_then(|index| text.as_bytes().get(index))
                .is_some_and(|byte| word(*byte));
            let after = text.as_bytes().get(end).is_some_and(|byte| word(*byte));
            if !before && !after {
                return Some(Span::new(span.start + start, span.start + end));
            }
            from = end;
        }
        None
    }
}
