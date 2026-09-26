use crate::error::ErrorKind;
use crate::ir::{ColorValue, Command, FormKind, Program, Target};

fn parse(source: &str) -> Program {
    match Program::parse(source) {
        Ok(program) => program,
        Err(errors) => panic!("{source:?} parses: {}", errors.render(source)),
    }
}

fn commands(source: &str) -> Vec<Command> {
    parse(source)
        .statements()
        .iter()
        .map(|statement| statement.command.clone())
        .collect()
}

fn first_error(source: &str) -> crate::CommandError {
    match Program::parse(source) {
        Ok(program) => panic!("{source:?} should fail, parsed {program:?}"),
        Err(errors) => match errors.0.into_iter().next() {
            Some(error) => error,
            None => panic!("an error"),
        },
    }
}

#[test]
fn statements_split_at_semicolons_and_lines_but_not_inside_quotes() {
    let program = parse("select a, name \"C;1\"; show cartoon, protein\n# a comment\nundo");
    assert_eq!(program.statements().len(), 3);
    assert!(matches!(program.statements()[2].command, Command::Undo));
}

#[test]
fn a_query_is_handed_to_molframe_verbatim() {
    let parsed = commands("select pocket, byres (within 5 of resname HEM)");
    let [Command::Select { name, query }] = parsed.as_slice() else {
        panic!("one select");
    };
    assert_eq!(name.as_str(), "pocket");
    let direct = molframe::Query::compile("byres (within 5 of resname HEM)")
        .map(|query| query.fingerprint())
        .ok();
    assert_eq!(Some(query.fingerprint()), direct);
}

#[test]
fn a_named_reference_is_kept_in_the_query() {
    let parsed = commands("show cartoon, $pocket and chain A");
    let Some(Command::Show(show)) = parsed.first() else {
        panic!("a show")
    };
    let Target::Query(query) = &show.target else {
        panic!("a query target")
    };
    assert_eq!(query.references(), vec!["pocket"]);
}

#[test]
fn show_reads_form_controls_layer_and_structure() {
    let parsed = commands(
        "show cartoon width=2 style=rocket color=red opacity=0.5 duplicate as main in s1, protein",
    );
    let Some(Command::Show(show)) = parsed.first() else {
        panic!("a show")
    };
    assert_eq!(show.form.kind(), FormKind::Cartoon);
    assert_eq!(show.layer.as_ref().map(crate::Name::as_str), Some("main"));
    assert_eq!(show.structure.as_ref().map(crate::Name::as_str), Some("s1"));
    assert!(show.duplicate);
    assert!(matches!(show.color, Some(ColorValue::Rgb { .. })));
    assert_eq!(show.opacity.map(crate::Opacity::get), Some(0.5));
}

#[test]
fn a_layer_target_is_written_with_an_at_sign() {
    assert!(matches!(
        commands("show @main").as_slice(),
        [Command::Reveal { layer }] if layer.as_str() == "main"
    ));
    assert!(matches!(
        commands("color chain, @main").as_slice(),
        [Command::Color { target: Target::Layer(layer), .. }] if layer.as_str() == "main"
    ));
}

#[test]
fn canonical_text_parses_back_to_the_same_command() {
    for source in [
        "select pocket, byres (within 5 of resname HEM)",
        "show ball_and_stick radius=0.3 as lig in s1, resname HEM",
        "color #ff3366, chain A",
        "color property plddt ramp=plasma domain=0:100, @main",
        "uncolor in s1, chain A",
        "opacity 0.4, @main",
        "focus $pocket",
        "hide @main",
        "unfocus",
    ] {
        let first = commands(source);
        let text = first
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        assert_eq!(commands(&text), first, "{source} -> {text}");
    }
}

#[test]
fn a_query_error_points_into_the_program() {
    let source = "show cartoon, protein and (chain A";
    let error = first_error(source);
    assert_eq!(error.kind, ErrorKind::Query);
    assert!(error.code.is_some());
    let span = error.span.unwrap_or_else(|| panic!("located"));
    assert!(
        span.start >= source.find("protein").unwrap_or(0),
        "{span:?}"
    );
    assert!(span.end <= source.len());
}

#[test]
fn an_unknown_verb_suggests_the_nearest_one() {
    let error = first_error("shwo cartoon, all");
    assert_eq!(error.kind, ErrorKind::Syntax);
    assert_eq!(error.suggestion.as_deref(), Some("show"));
    assert_eq!(error.span.map(|span| (span.start, span.end)), Some((0, 4)));
}

#[test]
fn an_unknown_control_names_the_forms_controls() {
    let error = first_error("show spacefill style=rocket, all");
    assert_eq!(error.kind, ErrorKind::InvalidOption);
    assert!(error.message.contains("radius"), "{}", error.message);
}

#[test]
fn every_failing_statement_is_reported() {
    let Err(errors) = Program::parse("shwo all\nshow cartoon, all\ncolor bleu, all") else {
        panic!("fails")
    };
    assert_eq!(errors.0.len(), 2);
    assert_eq!(errors.0[0].statement, Some(0));
    assert_eq!(errors.0[1].statement, Some(2));
    assert_eq!(errors.0[1].suggestion.as_deref(), Some("blue"));
}

#[test]
fn an_unclosed_quote_is_a_lexical_error() {
    assert_eq!(first_error("select a, name \"CA").kind, ErrorKind::Lexical);
}

#[test]
fn an_opacity_outside_zero_to_one_is_rejected() {
    assert_eq!(
        first_error("opacity 1.5, @main").kind,
        ErrorKind::InvalidOpacity
    );
}

#[test]
fn a_rendered_error_underlines_its_span() {
    let source = "show cartoon, all\nshow cartoom, all";
    let error = first_error(source);
    let rendered = error.render(source);
    assert!(rendered.contains("line 2, column 6"), "{rendered}");
    assert!(rendered.contains("^^^^^^^"), "{rendered}");
}
