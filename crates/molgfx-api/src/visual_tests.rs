use crate::{BoolExpr, Color, ColorExpr, Parameter, ScalarExpr, VisualStyle};

#[test]
fn scalar_constants_are_folded_before_shader_emission() {
    let opacity = ScalarExpr::from(0.25) + ScalarExpr::from(0.5);
    let style = VisualStyle {
        color: ColorExpr::Constant(Color::rgb(1, 2, 3)),
        opacity,
        visible: BoolExpr::Constant(true),
    };
    assert!(style.wgsl().contains("return 0.75000000"));
    assert!(!style.wgsl().contains(" + "));
}

#[test]
fn typed_parameters_have_stable_names_and_hashes() {
    let parameter = Parameter::new("focus_opacity", 0.3_f32);
    let style = VisualStyle {
        color: ColorExpr::Constant(Color::rgb(1, 2, 3)),
        opacity: parameter.into(),
        visible: BoolExpr::State("selected".into()),
    };
    assert!(style.wgsl().contains("parameters.focus_opacity"));
    assert_eq!(style.stable_hash(), style.clone().stable_hash());
    assert!(style.explain().contains("interpreter: false"));
}

#[test]
fn conflicting_parameter_declarations_are_rejected() {
    let first = Parameter::new("shared", 0.25_f32);
    let second = Parameter::new("shared", 0.75_f32);
    let style = VisualStyle::new(
        Color::rgb(10, 20, 30),
        ScalarExpr::from(first) + ScalarExpr::from(second),
        BoolExpr::Constant(true),
    );
    assert!(style.compile().is_err());
}

#[test]
fn compiler_reports_sorted_inputs_and_the_earliest_required_stage() {
    let style = VisualStyle::new(
        ColorExpr::Ramp {
            value: ScalarExpr::property("charge"),
            palette: "coolwarm".into(),
            domain: [-1.0, 1.0],
            missing: Color::rgb(128, 128, 128),
        },
        Parameter::new("opacity", 0.7_f32),
        BoolExpr::state("selected") | BoolExpr::Constant(true),
    );
    let compiled = style
        .compile()
        .unwrap_or_else(|error| panic!("style must compile: {error}"));
    assert_eq!(compiled.properties(), &[Box::<str>::from("charge")]);
    assert_eq!(compiled.parameters(), &[Box::<str>::from("opacity")]);
    assert_eq!(compiled.stage(), crate::visual::VisualStage::Fragment);
    assert!(compiled.wgsl().contains("fn visual_color()"));
    assert!(compiled.wgsl().contains("fn visual_visible()"));
}

#[test]
fn invalid_binding_names_never_reach_wgsl() {
    let style = VisualStyle::new(
        Color::rgb(1, 2, 3),
        ScalarExpr::property("x); discard; //"),
        BoolExpr::Constant(true),
    );
    assert!(style.compile().is_err());
}
