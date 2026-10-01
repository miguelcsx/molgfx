//! One statement: a verb, its arguments and its target.

use super::arguments::{Arguments, color_value, layer_word, name_word, target};
use super::targets::optional_target;
use super::words::{Word, comma_pieces, first_comma, trim, words};
use crate::error::{CommandError, ErrorKind, Span};
use crate::ir::{Command, Form, FormKind, MeasureKind, Opacity, OptionError, QueryText, Show};
use crate::registry;

/// Parses an explicit `interaction` statement, when the verb is that one.
///
/// Explicit interactions are caller-supplied API values: their JSON form is
/// accepted unchanged, with no molecular chemistry parsed or computed here.
fn interaction(source: &str, span: Span) -> Result<Option<Command>, CommandError> {
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

/// Parses the statement at `span`.
pub(super) fn parse(source: &str, span: Span) -> Result<Command, CommandError> {
    if let Some(command) = interaction(source, span)? {
        return Ok(command);
    }
    let (head, tail) = match first_comma(source, span) {
        Some(comma) => (
            Span::new(span.start, comma),
            Some(trim(source, Span::new(comma + 1, span.end))),
        ),
        None => (span, None),
    };
    let mut head_words = words(source, head);
    if head_words.is_empty() {
        return Err(syntax("a statement starts with a verb", span));
    }
    let verb = head_words.remove(0);
    let arguments = Arguments::new(head_words, verb.span);
    match verb.text {
        "select" => select(source, &arguments, tail),
        "unselect" => {
            let name = arguments.single("unselect NAME")?;
            no_target(tail, "unselect")?;
            Ok(Command::Unselect {
                name: name_word(name)?,
            })
        }
        "show" => show(source, &arguments, tail),
        "hide" | "remove" => {
            let layer = layer_word(arguments.single(&format!("{} @LAYER", verb.text))?)?;
            no_target(tail, verb.text)?;
            Ok(if verb.text == "hide" {
                Command::Hide { layer }
            } else {
                Command::Remove { layer }
            })
        }
        "label" => label(source, verb.span, span),
        "distance" | "angle" | "dihedral" => measure(source, verb, &arguments, tail),
        "color" => color(source, &arguments, tail),
        "uncolor" => super::targets::uncolor(source, &arguments, tail),
        "opacity" => opacity(source, &arguments, tail),
        "focus" => {
            let rest = trim(source, Span::new(verb.span.end, span.end));
            if rest.start == rest.end {
                return Err(syntax("focus needs a target: focus TARGET", verb.span));
            }
            no_target(tail, "focus")?;
            Ok(Command::Focus {
                target: target(source, rest)?,
            })
        }
        "auto" => {
            let structure = arguments
                .words()
                .first()
                .copied()
                .map(name_word)
                .transpose()?;
            if let Some(extra) = arguments.words().get(1) {
                return Err(syntax("auto takes at most one structure name", extra.span));
            }
            no_target(tail, "auto")?;
            Ok(Command::Auto { structure })
        }
        "pocket" => super::targets::pocket(source, &arguments, tail),
        "unfocus" | "undo" | "redo" => {
            if let Some(extra) = arguments.first() {
                return Err(syntax(
                    format!("{} takes no arguments", verb.text),
                    extra.span,
                ));
            }
            no_target(tail, verb.text)?;
            Ok(match verb.text {
                "unfocus" => Command::Unfocus,
                "undo" => Command::Undo,
                _ => Command::Redo,
            })
        }
        "assembly" => assembly(source, verb.span, span),
        "plane" => plane(source, verb.span, span),
        "volume" => volume(source, verb.span, span),
        unknown => Err(CommandError::new(
            ErrorKind::Syntax,
            format!("'{unknown}' is not a command"),
        )
        .at(verb.span)
        .suggest(registry::suggest(unknown, registry::verb_names()))),
    }
}

/// Parses an assembly JSON object, or `null` to remove the assembly.
fn assembly(source: &str, verb: Span, statement: Span) -> Result<Command, CommandError> {
    let payload = source[verb.end..statement.end].trim();
    if payload.is_empty() {
        return Err(syntax("assembly needs a JSON specification or null", verb));
    }
    let assembly = serde_json::from_str(payload)
        .map_err(|error| syntax(format!("invalid assembly specification: {error}"), verb))?;
    Ok(Command::Assembly { assembly })
}
/// Parses a planar-guide JSON object.
fn plane(source: &str, verb: Span, statement: Span) -> Result<Command, CommandError> {
    let payload = source[verb.end..statement.end].trim();
    if payload.is_empty() {
        return Err(syntax("plane needs a JSON plane specification", verb));
    }
    let plane = serde_json::from_str(payload)
        .map_err(|error| syntax(format!("invalid plane specification: {error}"), verb))?;
    Ok(Command::Plane { plane })
}

/// `volume {"source": {...}, "dimensions": [...], ...}`.
///
/// The volume's metadata is the same JSON the scene wire format uses, so a
/// caller can author a descriptor and pass it through unchanged rather than
/// naming every field twice.
fn volume(source: &str, verb: Span, statement: Span) -> Result<Command, CommandError> {
    let payload = source[verb.end..statement.end].trim();
    if payload.is_empty() {
        return Err(syntax(
            "volume needs a JSON volume specification: volume {\"source\":...}",
            verb,
        ));
    }
    let volume = serde_json::from_str(payload)
        .map_err(|error| syntax(format!("invalid volume specification: {error}"), verb))?;
    Ok(Command::Volume { volume })
}

/// `label "TEXT" [in STRUCTURE], QUERY`.
///
/// The text is quoted so it can hold spaces, commas and keywords; a single
/// bare word needs no quotes.
fn label(source: &str, verb: Span, statement: Span) -> Result<Command, CommandError> {
    const USAGE: &str = "label \"TEXT\" [in STRUCTURE], QUERY";
    let Some(comma) = first_comma(source, statement) else {
        return Err(syntax(format!("label needs a target: {USAGE}"), verb));
    };
    let head = trim(source, Span::new(verb.end, comma));
    let (text, rest) = label_text(source, head)?;
    let mut structure = None;
    let mut rest_words = words(source, rest).into_iter();
    while let Some(word) = rest_words.next() {
        if word.text == "in" {
            structure = Some(name_word(following(
                &mut rest_words,
                word,
                "in STRUCTURE",
            )?)?);
        } else {
            return Err(syntax(format!("unexpected '{}'", word.text), word.span));
        }
    }
    let tail = trim(source, Span::new(comma + 1, statement.end));
    if tail.start == tail.end {
        return Err(syntax("label needs a target after the comma", verb));
    }
    let target = QueryText::compile(&source[tail.start..tail.end])
        .map_err(|diagnostics| super::arguments::query_error(&diagnostics, tail))?;
    Ok(Command::Label {
        text,
        target,
        structure,
    })
}

/// The label text at the start of `head`, and the span after it.
fn label_text(source: &str, head: Span) -> Result<(String, Span), CommandError> {
    let text = &source[head.start..head.end];
    let Some(quote) = text.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
        let Some(word) = words(source, head).into_iter().next() else {
            return Err(syntax("label needs text: label \"TEXT\", QUERY", head));
        };
        return Ok((
            word.text.to_owned(),
            trim(source, Span::new(word.span.end, head.end)),
        ));
    };
    let mut value = String::new();
    let mut escaped = false;
    for (offset, character) in text.char_indices().skip(1) {
        match (escaped, character) {
            (true, other) => {
                value.push(other);
                escaped = false;
            }
            (false, '\\') => escaped = true,
            (false, other) if other == quote => {
                let end = head.start + offset + character.len_utf8();
                return Ok((value, trim(source, Span::new(end, head.end))));
            }
            (false, other) => value.push(other),
        }
    }
    Err(syntax("the label text is missing its closing quote", head))
}

/// `distance|angle|dihedral [in STRUCTURE], QUERY, QUERY[, QUERY[, QUERY]]`.
fn measure(
    source: &str,
    verb: Word<'_>,
    arguments: &Arguments<'_>,
    tail: Option<Span>,
) -> Result<Command, CommandError> {
    let kind = match verb.text {
        "distance" => MeasureKind::Distance,
        "angle" => MeasureKind::Angle,
        _ => MeasureKind::Dihedral,
    };
    let mut structure = None;
    let mut rest = arguments.words().iter().copied();
    while let Some(word) = rest.next() {
        if word.text == "in" {
            structure = Some(name_word(following(&mut rest, word, "in STRUCTURE")?)?);
        } else {
            return Err(syntax(format!("unexpected '{}'", word.text), word.span));
        }
    }
    let Some(tail) = tail else {
        return Err(syntax(
            format!(
                "{} needs {} points after a comma",
                kind.name(),
                kind.arity()
            ),
            verb.span,
        ));
    };
    let pieces = comma_pieces(source, tail);
    if pieces.len() != kind.arity() {
        return Err(syntax(
            format!(
                "{} takes {} points, one query each, and {} were given",
                kind.name(),
                kind.arity(),
                pieces.len()
            ),
            tail,
        ));
    }
    let mut points = Vec::with_capacity(pieces.len());
    for piece in pieces {
        if piece.start == piece.end {
            return Err(syntax("a point is missing its query", tail));
        }
        points.push(
            QueryText::compile(&source[piece.start..piece.end])
                .map_err(|diagnostics| super::arguments::query_error(&diagnostics, piece))?,
        );
    }
    Ok(Command::Measure {
        kind,
        points,
        structure,
    })
}

fn select(
    source: &str,
    arguments: &Arguments<'_>,
    tail: Option<Span>,
) -> Result<Command, CommandError> {
    let name = name_word(arguments.single("select NAME, QUERY")?)?;
    let Some(tail) = tail.filter(|tail| tail.start < tail.end) else {
        return Err(syntax(
            "select needs a query after a comma: select NAME, QUERY",
            arguments.anchor(),
        ));
    };
    let query = QueryText::compile(&source[tail.start..tail.end])
        .map_err(|diagnostics| super::arguments::query_error(&diagnostics, tail))?;
    Ok(Command::Select { name, query })
}

fn show(
    source: &str,
    arguments: &Arguments<'_>,
    tail: Option<Span>,
) -> Result<Command, CommandError> {
    let words = arguments.words();
    let Some(first) = words.first().copied() else {
        return Err(syntax(
            "show needs a form and a target: show FORM, TARGET",
            arguments.anchor(),
        ));
    };
    if first.text.starts_with('@') && tail.is_none() {
        if let Some(extra) = words.get(1) {
            return Err(syntax("show @LAYER takes nothing else", extra.span));
        }
        return Ok(Command::Reveal {
            layer: layer_word(first)?,
        });
    }
    let Some(kind) = FormKind::from_name(first.text) else {
        // `show protein` names a target where the form belongs; say how the
        // statement is written rather than only that the word is no form.
        let message = if tail.is_none() {
            format!(
                "'{}' is not a form; write the form first and the target after a comma: show cartoon, {}",
                first.text, first.text
            )
        } else {
            format!("'{}' is not a form", first.text)
        };
        return Err(CommandError::new(ErrorKind::Syntax, message)
            .at(first.span)
            .suggest(registry::suggest(first.text, registry::form_names())));
    };
    let target = optional_target(source, tail, first.span, &format!("show {}", kind.name()))?;
    let mut show = Show::new(Form::new(kind), target);
    let mut rest = words[1..].iter().copied();
    while let Some(word) = rest.next() {
        match word.text {
            "duplicate" => show.duplicate = true,
            "as" => show.layer = Some(name_word(following(&mut rest, word, "as LAYER")?)?),
            "in" => show.structure = Some(name_word(following(&mut rest, word, "in STRUCTURE")?)?),
            _ => show_control(&mut show, word)?,
        }
    }
    Ok(Command::Show(show))
}

fn show_control(show: &mut Show, word: Word<'_>) -> Result<(), CommandError> {
    let Some((key, value)) = word.text.split_once('=') else {
        return Err(syntax(
            format!(
                "'{}' is not an argument of show; write control=value, duplicate, as LAYER or in STRUCTURE",
                word.text
            ),
            word.span,
        ));
    };
    match key {
        "color" => {
            show.color = Some(color_value(value, word.span)?);
            Ok(())
        }
        "opacity" => {
            show.opacity = Some(opacity_value(value, word.span)?);
            Ok(())
        }
        _ => match show.form.set_option(key, value) {
            Ok(()) => Ok(()),
            Err(OptionError::Unknown { known }) => {
                let names = known.iter().map(|option| option.name);
                let listed: Vec<&str> = names.clone().chain(["color", "opacity"]).collect();
                Err(CommandError::new(
                    ErrorKind::InvalidOption,
                    format!(
                        "{} has no control '{key}'; its controls are {}",
                        show.form.kind().name(),
                        listed.join(", ")
                    ),
                )
                .at(word.span)
                .suggest(registry::suggest(key, listed.iter().copied())))
            }
            Err(OptionError::Value(reason)) => Err(CommandError::new(
                ErrorKind::InvalidOption,
                format!("{key}: {reason}"),
            )
            .at(word.span)),
        },
    }
}

fn color(
    source: &str,
    arguments: &Arguments<'_>,
    tail: Option<Span>,
) -> Result<Command, CommandError> {
    let words = arguments.words();
    let Some(first) = words.first().copied() else {
        return Err(syntax(
            "color needs a colour and a target: color COLOR, TARGET",
            arguments.anchor(),
        ));
    };
    let mut rest = words[1..].iter().copied().peekable();
    let mut color = if first.text == "property" {
        super::arguments::property_color(first, &mut rest)?
    } else {
        color_value(first.text, first.span)?
    };
    let mut structure = None;
    while let Some(word) = rest.next() {
        if word.text == "in" {
            structure = Some(name_word(following(&mut rest, word, "in STRUCTURE")?)?);
        } else if word.text.contains('=') {
            super::arguments::color_option(&mut color, word)?;
        } else {
            return Err(syntax(format!("unexpected '{}'", word.text), word.span));
        }
    }
    Ok(Command::Color {
        color,
        target: optional_target(source, tail, first.span, "color")?,
        structure,
    })
}

fn opacity(
    source: &str,
    arguments: &Arguments<'_>,
    tail: Option<Span>,
) -> Result<Command, CommandError> {
    let value = arguments.single("opacity VALUE, @LAYER")?;
    let Some(tail) = tail.filter(|tail| tail.start < tail.end) else {
        return Err(syntax(
            "opacity needs a layer after a comma: opacity VALUE, @LAYER",
            value.span,
        ));
    };
    let layer = Word {
        text: &source[tail.start..tail.end],
        span: tail,
    };
    Ok(Command::Opacity {
        value: opacity_value(value.text, value.span)?,
        layer: layer_word(layer)?,
    })
}

fn opacity_value(text: &str, span: Span) -> Result<Opacity, CommandError> {
    text.parse::<f32>()
        .ok()
        .and_then(|value| Opacity::new(value).ok())
        .ok_or_else(|| {
            CommandError::new(
                ErrorKind::InvalidOpacity,
                format!("'{text}' is not an opacity; write a number from 0 to 1"),
            )
            .at(span)
        })
}

pub(super) fn following<'a>(
    rest: &mut impl Iterator<Item = Word<'a>>,
    keyword: Word<'a>,
    usage: &str,
) -> Result<Word<'a>, CommandError> {
    rest.next().ok_or_else(|| {
        syntax(
            format!("'{}' needs a name: {usage}", keyword.text),
            keyword.span,
        )
    })
}

fn no_target(tail: Option<Span>, verb: &str) -> Result<(), CommandError> {
    match tail {
        Some(tail) => Err(syntax(format!("{verb} takes no target"), tail)),
        None => Ok(()),
    }
}

pub(super) fn syntax(message: impl Into<String>, span: Span) -> CommandError {
    CommandError::new(ErrorKind::Syntax, message).at(span)
}
