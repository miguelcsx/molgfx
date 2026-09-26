//! Ordered commands executed as one unit.

use super::command::Command;
use crate::error::{CommandErrors, Span};
use serde::Serialize;

/// One command and where it was written.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Statement {
    /// The command.
    pub command: Command,
    /// Its source range, when it was parsed from text.
    pub span: Option<Span>,
}

/// An ordered sequence of commands.
///
/// A program executes atomically: every statement is planned against the
/// state the earlier ones leave, and either the whole program commits as one
/// scene revision or nothing changes.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Program {
    source: Option<Box<str>>,
    statements: Vec<Statement>,
}

impl Program {
    /// Parses command text.
    ///
    /// # Errors
    ///
    /// Returns every statement's error, each located in `source`. Parsing
    /// continues past a bad statement so all of them are reported at once.
    pub fn parse(source: &str) -> Result<Self, CommandErrors> {
        crate::language::parse_program(source)
    }

    /// A program of typed commands, which carry no source text.
    #[must_use]
    pub fn from_commands(commands: impl IntoIterator<Item = Command>) -> Self {
        Self {
            source: None,
            statements: commands
                .into_iter()
                .map(|command| Statement {
                    command,
                    span: None,
                })
                .collect(),
        }
    }

    pub(crate) fn from_parts(source: &str, statements: Vec<Statement>) -> Self {
        Self {
            source: Some(source.into()),
            statements,
        }
    }

    /// The text this program was parsed from, if any.
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// The statements, in order.
    #[must_use]
    pub fn statements(&self) -> &[Statement] {
        &self.statements
    }

    /// Whether the program has no statement.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    /// One line per statement, in canonical command text.
    #[must_use]
    pub fn explain(&self) -> String {
        self.statements
            .iter()
            .enumerate()
            .map(|(index, statement)| format!("{}: {}", index + 1, statement.command))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl From<Command> for Program {
    fn from(command: Command) -> Self {
        Self::from_commands([command])
    }
}
