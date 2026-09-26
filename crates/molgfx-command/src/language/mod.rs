//! Command text to typed commands.
//!
//! Text is split into statements at `;` and newlines outside quotes; a
//! statement that starts with `#` is a comment. Each statement is a verb, its
//! arguments, and — after the first comma outside quotes — its target. A
//! target is `@name` for a layer or otherwise a `MolFrame` query, which is
//! handed to `MolFrame` exactly as written, so its diagnostics point into the
//! author's text.

mod arguments;
mod split;
mod statement;
mod words;

use crate::error::{CommandError, CommandErrors};
use crate::ir::{Program, Statement};

/// Parses every statement, reporting all the statements that fail.
pub(crate) fn parse_program(source: &str) -> Result<Program, CommandErrors> {
    let mut statements = Vec::new();
    let mut errors: Vec<CommandError> = Vec::new();
    for span in split::statements(source)? {
        let index = statements.len() + errors.len();
        match statement::parse(source, span) {
            Ok(command) => statements.push(Statement {
                command,
                span: Some(span),
            }),
            Err(error) => errors.push(error.in_statement(index, Some(span))),
        }
    }
    if errors.is_empty() {
        Ok(Program::from_parts(source, statements))
    } else {
        Err(CommandErrors(errors))
    }
}

#[cfg(test)]
mod tests;
