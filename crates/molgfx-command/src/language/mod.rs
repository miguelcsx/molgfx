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
mod targets;
mod words;

#[cfg(test)]
mod tests;

mod program;

pub(crate) use program::parse_program;
