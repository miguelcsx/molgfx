//! Transport commands: `seek`, `play`, `pause` and `dilate`.
//!
//! Every time is written as a number and a unit symbol. A missing or unknown
//! unit is a syntax error; there is no default to fall back to.

use super::arguments::{Arguments, name_word};
use super::statement::{no_target, syntax};
use crate::error::{CommandError, Span};
use crate::ir::{Command, TimeAmount, Transport};

pub(super) fn parse(
    verb: &str,
    arguments: &Arguments<'_>,
    tail: Option<Span>,
) -> Result<Command, CommandError> {
    no_target(tail, verb)?;
    let words = arguments.words();
    let transport = match (verb, words) {
        ("play", []) => Transport::Play,
        ("pause", []) => Transport::Pause,
        ("play" | "pause", [extra, ..]) => {
            return Err(syntax(format!("{verb} takes no arguments"), extra.span));
        }
        ("seek", [value, unit]) => Transport::Seek {
            time: amount(value.text, unit.text, value.span.start, unit.span.end)?,
        },
        ("dilate", [track, value, unit]) => Transport::Dilate {
            track: name_word(*track)?,
            rate: amount(value.text, unit.text, value.span.start, unit.span.end)?,
        },
        ("seek", _) => {
            return Err(syntax(
                "seek needs a time and a unit: seek TIME UNIT",
                arguments.anchor(),
            ));
        }
        _ => {
            return Err(syntax(
                "dilate needs a track, a rate and a unit: dilate TRACK RATE UNIT",
                arguments.anchor(),
            ));
        }
    };
    Ok(Command::Transport(transport))
}

fn amount(value: &str, unit: &str, start: usize, end: usize) -> Result<TimeAmount, CommandError> {
    TimeAmount::parse(value, unit).map_err(|message| syntax(message, Span::new(start, end)))
}
