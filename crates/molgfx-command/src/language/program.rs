//! Parsing a program of statements.

use super::{split, statement};
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
