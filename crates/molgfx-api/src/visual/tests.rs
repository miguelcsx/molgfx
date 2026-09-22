use crate::{BoolExpr, Color, ColorExpr, Parameter, ScalarExpr, VisualStyle};
use std::sync::Arc;

fn scalar_property(name: &str) -> crate::ScalarProperty {
    crate::ScalarPropertyBinding::new(
        crate::StructureId::new(1),
        name,
        crate::DataSource::new("test-property"),
        Arc::from([0.0_f32, 1.0]),
    )
    .into_parts()
    .0
}

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
    // The compiler reports what it produced; the executed path is the
    // renderer's to report, so this must not claim a pipeline ran.
    assert!(style.explain().contains("specialized-wgsl: emitted"));
    assert!(!style.explain().contains("interpreter: true"));
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
            value: ScalarExpr::property(scalar_property("charge")).into(),
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
        ScalarExpr::property(scalar_property("x); discard; //")),
        BoolExpr::Constant(true),
    );
    assert!(style.compile().is_err());
}

#[test]
fn composing_an_expression_retains_shared_child_nodes() {
    let shared = Arc::new(ScalarExpr::property(scalar_property("charge")));
    let expression = ScalarExpr::Add(Arc::clone(&shared), Arc::new(ScalarExpr::Constant(1.0)));
    let clone = expression.clone();
    let ScalarExpr::Add(original_left, _) = expression else {
        panic!("addition expected")
    };
    let ScalarExpr::Add(cloned_left, _) = clone else {
        panic!("addition expected")
    };
    assert!(Arc::ptr_eq(&shared, &original_left));
    assert!(Arc::ptr_eq(&original_left, &cloned_left));
}

#[test]
fn structurally_equal_subtrees_share_one_lowered_node() {
    use crate::visual::intern::Interner;
    let shared = ScalarExpr::from(0.5) * ScalarExpr::input("time");
    let left = shared.clone() + ScalarExpr::from(1.0);
    let right = shared + ScalarExpr::from(1.0);
    let mut interner = Interner::default();
    assert_eq!(interner.scalar(&left), interner.scalar(&right));
}

#[test]
fn differently_shaped_nodes_do_not_collide() {
    use crate::visual::intern::Interner;
    let a = ScalarExpr::from(2.0) + ScalarExpr::input("time");
    let b = ScalarExpr::from(2.0) * ScalarExpr::input("time");
    let mut interner = Interner::default();
    assert_ne!(interner.scalar(&a), interner.scalar(&b));
}

fn nested(depth: usize) -> VisualStyle {
    let mut expression = ScalarExpr::input("time");
    for _ in 0..depth {
        expression = expression * ScalarExpr::from(0.999);
    }
    VisualStyle {
        color: ColorExpr::Constant(Color::rgb(1, 2, 3)),
        opacity: expression,
        visible: BoolExpr::Constant(true),
    }
}

#[test]
fn a_deep_expression_graph_compiles_without_quadratic_cost() {
    // Keying subexpressions by their serialized form made this grow with the
    // square of the depth, so a graph at the depth limit took visibly long.
    let start = std::time::Instant::now();
    let Ok(_) = nested(crate::visual::intern::MAX_DEPTH - 8).compile() else {
        panic!("a graph within the depth limit must compile")
    };
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "lowering at the depth limit must not take seconds"
    );
}

#[test]
fn a_graph_past_the_depth_limit_is_an_error_not_a_crash() {
    let Err(error) = nested(crate::visual::intern::MAX_DEPTH + 8).compile() else {
        panic!("an over-deep graph must be rejected")
    };
    assert!(error.to_string().contains("nests deeper"), "{error}");
}
