//! Argument words and targets.

use super::statement::syntax;
use super::words::Word;
use crate::error::{CommandError, ErrorKind, Span};
use crate::ir::{ColorValue, Name, QueryText, Target};
use crate::registry;

/// The words after a verb, and where the verb was.
pub(super) struct Arguments<'a> {
    words: Vec<Word<'a>>,
    verb: Span,
}

impl<'a> Arguments<'a> {
    pub(super) const fn new(words: Vec<Word<'a>>, verb: Span) -> Self {
        Self { words, verb }
    }

    pub(super) fn words(&self) -> &[Word<'a>] {
        &self.words
    }

    pub(super) fn first(&self) -> Option<Word<'a>> {
        self.words.first().copied()
    }

    /// Where to point when an argument is missing.
    pub(super) const fn anchor(&self) -> Span {
        self.verb
    }

    /// Exactly one argument.
    pub(super) fn single(&self, usage: &str) -> Result<Word<'a>, CommandError> {
        match self.words.as_slice() {
            [word] => Ok(*word),
            [] => Err(syntax(format!("missing argument: {usage}"), self.verb)),
            [_, extra, ..] => Err(syntax(format!("too many arguments: {usage}"), extra.span)),
        }
    }
}

/// A word that must be a name.
pub(super) fn name_word(word: Word<'_>) -> Result<Name, CommandError> {
    Name::new(word.text).map_err(|reason| {
        syntax(
            format!("'{}' is not a valid name: {reason}", word.text),
            word.span,
        )
    })
}

/// A layer reference, written `@name` or plainly `name`.
pub(super) fn layer_word(word: Word<'_>) -> Result<Name, CommandError> {
    let text = word
        .text
        .strip_prefix('@')
        .map_or(word.text, str::trim_start);
    name_word(Word {
        text,
        span: word.span,
    })
}

/// A target: `@layer`, or a `MolFrame` query compiled from exactly `span`.
pub(super) fn target(source: &str, span: Span) -> Result<Target, CommandError> {
    let text = &source[span.start..span.end];
    if let Some(layer) = text.strip_prefix('@') {
        return name_word(Word { text: layer, span }).map(Target::Layer);
    }
    QueryText::compile(text)
        .map(Target::Query)
        .map_err(|diagnostics| query_error(&diagnostics, span))
}

/// `MolFrame`'s first diagnostic, relocated from the query into the program.
pub(super) fn query_error(diagnostics: &[molframe::Diagnostic], query: Span) -> CommandError {
    let Some(first) = diagnostics.first() else {
        return CommandError::new(ErrorKind::Query, "the query is not valid").at(query);
    };
    let span = first.span().map_or(query, |span| {
        let length = query.end - query.start;
        let clamp =
            |offset: u64| usize::try_from(offset).map_or(length, |offset| offset.min(length));
        let start = clamp(span.start.byte_offset);
        let end = clamp(span.end).max(start);
        Span::new(query.start + start, query.start + end)
    });
    let mut error = CommandError::new(ErrorKind::Query, first.message().to_owned()).at(span);
    error.code = Some(first.code().to_string());
    error
}

/// A colour written as one word: a scheme, a named colour or `#rrggbb`.
pub(super) fn color_value(text: &str, span: Span) -> Result<ColorValue, CommandError> {
    if text.starts_with('#') {
        return ColorValue::hex(text)
            .map_err(|reason| CommandError::new(ErrorKind::InvalidColor, reason).at(span));
    }
    ColorValue::named(text).ok_or_else(|| {
        CommandError::new(
            ErrorKind::InvalidColor,
            format!("'{text}' is not a colour; write a scheme, a colour name or #rrggbb"),
        )
        .at(span)
        .suggest(registry::suggest(text, registry::color_words()))
    })
}

/// `property NAME [ramp=RAMP] [domain=LOW:HIGH]`, after the `property` word.
pub(super) fn property_color<'a>(
    keyword: Word<'a>,
    rest: &mut std::iter::Peekable<impl Iterator<Item = Word<'a>>>,
) -> Result<ColorValue, CommandError> {
    let Some(property) = rest.next() else {
        return Err(syntax(
            "property needs a name: color property NAME [ramp=RAMP] [domain=LOW:HIGH], @LAYER",
            keyword.span,
        ));
    };
    let mut ramp: Box<str> = "viridis".into();
    let mut domain = None;
    while let Some(word) = rest.next_if(|word| word.text.contains('=')) {
        match word.text.split_once('=') {
            Some(("ramp", value)) => {
                if !registry::RAMPS.contains(&value) {
                    return Err(CommandError::new(
                        ErrorKind::InvalidColor,
                        format!(
                            "'{value}' is not a ramp; ramps are {}",
                            registry::RAMPS.join(", ")
                        ),
                    )
                    .at(word.span)
                    .suggest(registry::suggest(value, registry::RAMPS.iter().copied())));
                }
                ramp = value.into();
            }
            Some(("domain", value)) => domain = Some(domain_value(value, word.span)?),
            _ => {
                return Err(CommandError::new(
                    ErrorKind::InvalidOption,
                    format!(
                        "'{}' is not a property colour control; use ramp= or domain=",
                        word.text
                    ),
                )
                .at(word.span));
            }
        }
    }
    Ok(ColorValue::Property {
        property: property.text.into(),
        ramp,
        domain,
    })
}

fn domain_value(text: &str, span: Span) -> Result<[f32; 2], CommandError> {
    let bounds = text
        .split_once(':')
        .and_then(|(low, high)| Some([low.parse::<f32>().ok()?, high.parse::<f32>().ok()?]));
    match bounds {
        Some([low, high]) if low.is_finite() && high.is_finite() && low < high => Ok([low, high]),
        _ => Err(CommandError::new(
            ErrorKind::InvalidColor,
            format!("'{text}' is not a domain; write LOW:HIGH with LOW below HIGH"),
        )
        .at(span)),
    }
}
