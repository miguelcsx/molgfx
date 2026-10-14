use crate::ErrorKind;
use crate::ir::{Command, Program, TimeAmount, Transport};
use molgfx_scene::time::TimeUnit;

fn one(source: &str) -> Transport {
    let program = match Program::parse(source) {
        Ok(program) => program,
        Err(errors) => panic!("{source:?} parses: {}", errors.render(source)),
    };
    match program.statements() {
        [statement] => match &statement.command {
            Command::Transport(transport) => transport.clone(),
            other => panic!("{source:?} is not a transport command: {other:?}"),
        },
        other => panic!("one statement expected, got {other:?}"),
    }
}

fn rejected(source: &str) -> crate::CommandError {
    match Program::parse(source) {
        Ok(program) => panic!("{source:?} should fail, parsed {program:?}"),
        Err(errors) => match errors.0.into_iter().next() {
            Some(error) => error,
            None => panic!("an error"),
        },
    }
}

fn amount(value: f64, unit: TimeUnit) -> TimeAmount {
    match TimeAmount::new(value, unit) {
        Ok(amount) => amount,
        Err(reason) => panic!("{reason}"),
    }
}

#[test]
fn seek_parses_a_time_and_its_unit() {
    assert_eq!(
        one("seek 2.5 s"),
        Transport::Seek {
            time: amount(2.5, TimeUnit::Second)
        }
    );
    assert_eq!(
        one("seek 40 ns"),
        Transport::Seek {
            time: amount(40.0, TimeUnit::Nanosecond)
        }
    );
}

#[test]
fn play_and_pause_take_no_arguments() {
    assert_eq!(one("play"), Transport::Play);
    assert_eq!(one("pause"), Transport::Pause);
    assert_eq!(rejected("play now").kind, ErrorKind::Syntax);
    assert_eq!(rejected("pause 1 s").kind, ErrorKind::Syntax);
}

#[test]
fn dilate_parses_a_track_a_rate_and_its_unit() {
    let Transport::Dilate { track, rate } = one("dilate fast 5 ns") else {
        panic!("a dilate");
    };
    assert_eq!(track.as_str(), "fast");
    assert_eq!(rate, amount(5.0, TimeUnit::Nanosecond));
}

#[test]
fn a_missing_unit_is_an_error_and_never_a_default() {
    assert_eq!(rejected("seek 2.5").kind, ErrorKind::Syntax);
    assert_eq!(rejected("dilate fast 5").kind, ErrorKind::Syntax);
    assert_eq!(rejected("seek").kind, ErrorKind::Syntax);
}

#[test]
fn an_unknown_unit_is_an_error() {
    let error = rejected("seek 2.5 minutes");
    assert_eq!(error.kind, ErrorKind::Syntax);
    assert!(error.message.contains("minutes"), "{}", error.message);
    assert_eq!(rejected("dilate fast 5 parsecs").kind, ErrorKind::Syntax);
}

#[test]
fn a_time_that_is_not_a_finite_number_is_an_error() {
    assert_eq!(rejected("seek soon s").kind, ErrorKind::Syntax);
    assert_eq!(rejected("seek inf s").kind, ErrorKind::Syntax);
    assert_eq!(rejected("seek NaN ns").kind, ErrorKind::Syntax);
}

#[test]
fn a_transport_command_does_not_take_a_target() {
    assert_eq!(rejected("seek 1 s, chain A").kind, ErrorKind::Syntax);
}

#[test]
fn canonical_text_parses_back_to_the_same_command() {
    for source in [
        "seek 2.5 s",
        "play",
        "pause",
        "dilate fast 5 ns",
        "seek 3 us",
    ] {
        let program = Program::parse(source).map_err(|_| source).ok();
        let Some(program) = program else {
            panic!("{source} parses");
        };
        let text = program.statements()[0].command.to_string();
        assert_eq!(one(&text), one(source), "{text}");
    }
}

#[test]
fn a_transport_command_round_trips_through_json() {
    let command = Command::Transport(Transport::Dilate {
        track: crate::Name::new("fast").unwrap_or_else(|_| panic!("a name")),
        rate: amount(5.0, TimeUnit::Nanosecond),
    });
    let json = serde_json::to_string(&command).unwrap_or_else(|_| panic!("serializes"));
    let back: Command = serde_json::from_str(&json).unwrap_or_else(|_| panic!("reads {json}"));
    assert_eq!(back, command);
}
