//! Parsing caller-supplied explicit interactions.

use super::statement::syntax;
use super::words::words;
use crate::error::{CommandError, Span};
use crate::ir::Command;

/// Parses an explicit interaction without interpreting molecular chemistry.
pub(super) fn interaction(source: &str, span: Span) -> Result<Option<Command>, CommandError> {
    let initial = words(source, span);
    let Some(verb) = initial.first() else {
        return Ok(None);
    };
    if verb.text != "interaction" {
        return Ok(None);
    }
    let payload = source[verb.span.end..span.end].trim();
    if payload.is_empty() {
        return Err(syntax(
            "interaction needs an explicit JSON specification",
            verb.span,
        ));
    }
    let interaction = serde_json::from_str(payload).map_err(|error| {
        syntax(
            format!("invalid interaction specification: {error}"),
            verb.span,
        )
    })?;
    Ok(Some(Command::Interaction { interaction }))
}
