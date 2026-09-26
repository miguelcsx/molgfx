//! Typed command and session errors, located in the source they came from.

use serde::Serialize;
use std::fmt;
use std::fmt::Write as _;

/// A half-open byte range of command source.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize)]
pub struct Span {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

impl Span {
    /// The span `start..end`.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// The 1-based line and column of this span's first byte in `source`.
    #[must_use]
    pub fn location(self, source: &str) -> (usize, usize) {
        let before = match source.get(..self.start) {
            Some(prefix) => prefix,
            None => source,
        };
        let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = match before.rfind('\n') {
            Some(newline) => before.len() - newline,
            None => before.len() + 1,
        };
        (line, column)
    }
}

/// What went wrong, as a stable category.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The text could not be split into tokens (an unclosed quote).
    Lexical,
    /// The tokens do not form a command.
    Syntax,
    /// `MolFrame` rejected a query.
    Query,
    /// A name refers to nothing.
    UnknownSymbol,
    /// A name refers to something of another kind.
    WrongSymbolKind,
    /// Named selections refer to each other in a cycle.
    Cycle,
    /// A form control is unknown or its value invalid.
    InvalidOption,
    /// A colour is not valid.
    InvalidColor,
    /// An opacity is outside zero to one.
    InvalidOpacity,
    /// A scene has several structures and the command names none.
    AmbiguousStructure,
    /// Something else still depends on what the command would remove.
    InUse,
    /// A layer name is taken by a different layer.
    Conflict,
    /// The scene changed outside the session since the session last edited it.
    StaleSession,
    /// There is nothing to undo or redo.
    History,
    /// The scene rejected the resulting edit.
    Scene,
}

/// One located, typed error.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct CommandError {
    /// The category.
    pub kind: ErrorKind,
    /// A concise, actionable description.
    pub message: String,
    /// Where in the source the error is, when it came from text.
    pub span: Option<Span>,
    /// Index of the statement, within its program, that failed.
    pub statement: Option<usize>,
    /// A near spelling of an unknown word, when one exists.
    pub suggestion: Option<String>,
    /// The `MolFrame` diagnostic code of a query error.
    pub code: Option<String>,
}

impl CommandError {
    /// An error of `kind`, with no location.
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            span: None,
            statement: None,
            suggestion: None,
            code: None,
        }
    }

    /// Locates this error at `span`.
    #[must_use]
    pub const fn at(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Attaches a near spelling.
    #[must_use]
    pub fn suggest(mut self, suggestion: Option<String>) -> Self {
        self.suggestion = suggestion;
        self
    }

    /// Records which statement failed, and locates the error at it when the
    /// error has no finer location of its own.
    #[must_use]
    pub const fn in_statement(mut self, index: usize, span: Option<Span>) -> Self {
        self.statement = Some(index);
        if self.span.is_none() {
            self.span = span;
        }
        self
    }

    /// The error with the offending source line underlined, for a console.
    #[must_use]
    pub fn render(&self, source: &str) -> String {
        let mut text = format!("error: {self}");
        let Some(span) = self.span else {
            return text;
        };
        let (line, column) = span.location(source);
        let Some(line_text) = source.lines().nth(line - 1) else {
            return text;
        };
        let width = span
            .end
            .saturating_sub(span.start)
            .clamp(1, line_text.len().max(1));
        let _ = write!(
            text,
            "\n  --> line {line}, column {column}\n   | {line_text}\n   | {}{}",
            " ".repeat(column - 1),
            "^".repeat(width)
        );
        text
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)?;
        if let Some(code) = &self.code {
            write!(formatter, " [{code}]")?;
        }
        if let Some(suggestion) = &self.suggestion {
            write!(formatter, "; did you mean '{suggestion}'?")?;
        }
        Ok(())
    }
}

impl std::error::Error for CommandError {}

/// Every error one program produced, in source order.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct CommandErrors(pub Vec<CommandError>);

impl CommandErrors {
    /// A single error.
    #[must_use]
    pub fn one(error: CommandError) -> Self {
        Self(vec![error])
    }

    /// The errors.
    #[must_use]
    pub fn as_slice(&self) -> &[CommandError] {
        &self.0
    }

    /// Every error rendered against `source`, one after another.
    #[must_use]
    pub fn render(&self, source: &str) -> String {
        self.0
            .iter()
            .map(|error| error.render(source))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl fmt::Display for CommandErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let messages: Vec<String> = self.0.iter().map(ToString::to_string).collect();
        formatter.write_str(&messages.join("; "))
    }
}

impl std::error::Error for CommandErrors {}

impl From<CommandError> for CommandErrors {
    fn from(error: CommandError) -> Self {
        Self::one(error)
    }
}
