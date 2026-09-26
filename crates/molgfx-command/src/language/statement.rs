//! One statement: a verb, its arguments and its target.

use super::arguments::{Arguments, color_value, layer_word, name_word, target};
use super::words::{Word, first_comma, trim, words};
use crate::error::{CommandError, ErrorKind, Span};
use crate::ir::{Command, Form, FormKind, Opacity, OptionError, QueryText, Show};
use crate::registry;

/// Parses the statement at `span`.
pub(super) fn parse(source: &str, span: Span) -> Result<Command, CommandError> {
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
        "color" => color(source, &arguments, tail),
        "uncolor" => uncolor(source, &arguments, tail),
        "opacity" => opacity(source, &arguments, tail),
        "focus" => {
            let rest = trim(source, Span::new(verb.span.end, span.end));
            if rest.start == rest.end {
                return Err(syntax("focus needs a target: focus TARGET", verb.span));
            }
            Ok(Command::Focus {
                target: target(source, rest)?,
            })
        }
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
        unknown => Err(CommandError::new(
            ErrorKind::Syntax,
            format!("'{unknown}' is not a command"),
        )
        .at(verb.span)
        .suggest(registry::suggest(unknown, registry::verb_names()))),
    }
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
        return Err(CommandError::new(
            ErrorKind::Syntax,
            format!("'{}' is not a form", first.text),
        )
        .at(first.span)
        .suggest(registry::suggest(first.text, registry::form_names())));
    };
    let Some(tail) = tail.filter(|tail| tail.start < tail.end) else {
        return Err(syntax(
            format!("show {} needs a target after a comma", kind.name()),
            first.span,
        ));
    };
    let mut show = Show::new(Form::new(kind), target(source, tail)?);
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
    let color = if first.text == "property" {
        super::arguments::property_color(first, &mut rest)?
    } else {
        color_value(first.text, first.span)?
    };
    let mut structure = None;
    while let Some(word) = rest.next() {
        if word.text == "in" {
            structure = Some(name_word(following(&mut rest, word, "in STRUCTURE")?)?);
        } else {
            return Err(syntax(format!("unexpected '{}'", word.text), word.span));
        }
    }
    let Some(tail) = tail.filter(|tail| tail.start < tail.end) else {
        return Err(syntax("color needs a target after a comma", first.span));
    };
    Ok(Command::Color {
        color,
        target: target(source, tail)?,
        structure,
    })
}

fn uncolor(
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
                .map_err(|diagnostics| super::arguments::query_error(&diagnostics, tail))?,
        ),
        None => None,
    };
    Ok(Command::Uncolor { target, structure })
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

fn following<'a>(
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
