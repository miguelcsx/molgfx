//! Statement arguments shared by more than one verb: the optional target after
//! the comma, and the colour-rule removal that has no colour to parse.
//!
//! As in `PyMOL`, a statement with no comma applies to everything; a comma with
//! nothing after it is still an error, because it reads as a target left out by
//! mistake rather than a request for the whole scene.

use super::arguments::{Arguments, name_word, query_error, target};
use super::statement::{following, syntax};
use crate::error::{CommandError, Span};
use crate::ir::{Command, Positive, QueryText, Target};

/// The target after a statement's comma, or everything when there is no comma.
///
/// # Errors
///
/// Returns a syntax error for a comma with nothing after it, and a query error
/// when the default target cannot be compiled.
pub(super) fn optional_target(
    source: &str,
    tail: Option<Span>,
    anchor: Span,
    statement: &str,
) -> Result<Target, CommandError> {
    match tail {
        None => QueryText::compile("all")
            .map(Target::Query)
            .map_err(|diagnostics| query_error(&diagnostics, anchor)),
        Some(tail) if tail.start < tail.end => target(source, tail),
        Some(_) => Err(syntax(
            format!("{statement} needs a target after the comma"),
            anchor,
        )),
    }
}

/// `uncolor [in STRUCTURE], [QUERY]`, with the structure before the comma.
pub(super) fn uncolor(
    source: &str,
    arguments: &Arguments<'_>,
    tail: Option<Span>,
) -> Result<Command, CommandError> {
    let mut structure = None;
    let mut rest = arguments.words().iter().copied();
    while let Some(word) = rest.next() {
        if word.text == "in" {
            structure = Some(name_word(following(&mut rest, word, "in STRUCTURE")?)?);
        } else {
            return Err(syntax(format!("unexpected '{}'", word.text), word.span));
        }
    }
    let target = match tail.filter(|tail| tail.start < tail.end) {
        Some(tail) => Some(
            QueryText::compile(&source[tail.start..tail.end])
                .map_err(|diagnostics| query_error(&diagnostics, tail))?,
        ),
        None => None,
    };
    Ok(Command::Uncolor { target, structure })
}

/// `pocket [near=N] [mid=M] [in STRUCTURE], QUERY`: the subject follows the comma.
pub(super) fn pocket(
    source: &str,
    arguments: &Arguments<'_>,
    tail: Option<Span>,
) -> Result<Command, CommandError> {
    let (mut near, mut mid, mut structure) = (None, None, None);
    let mut rest = arguments.words().iter().copied();
    while let Some(word) = rest.next() {
        if word.text == "in" {
            structure = Some(name_word(following(&mut rest, word, "in STRUCTURE")?)?);
            continue;
        }
        let radius = |value: &str| {
            value
                .parse::<f32>()
                .map_err(|_| "a number")
                .and_then(Positive::new)
                .map_err(|reason| syntax(format!("pocket radii must be {reason}"), word.span))
        };
        match word.text.split_once('=') {
            Some(("near", value)) => near = Some(radius(value)?),
            Some(("mid", value)) => mid = Some(radius(value)?),
            _ => {
                return Err(syntax(
                    format!(
                        "'{}' is not an argument of pocket; write near=N, mid=M or in STRUCTURE",
                        word.text
                    ),
                    word.span,
                ));
            }
        }
    }
    let Some(tail) = tail.filter(|tail| tail.start < tail.end) else {
        return Err(syntax(
            "pocket needs a subject after the comma: pocket, QUERY",
            arguments.anchor(),
        ));
    };
    let target = QueryText::compile(&source[tail.start..tail.end])
        .map_err(|diagnostics| query_error(&diagnostics, tail))?;
    Ok(Command::Pocket {
        target,
        near,
        mid,
        structure,
    })
}
