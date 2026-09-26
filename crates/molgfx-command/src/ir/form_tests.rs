use super::{Form, FormKind, OptionError, OptionKind};

#[test]
fn every_form_name_round_trips_through_its_kind() {
    for kind in FormKind::ALL {
        assert_eq!(FormKind::from_name(kind.name()), Some(*kind));
    }
}

#[test]
fn a_control_belongs_to_one_form() {
    let mut cartoon = Form::new(FormKind::Cartoon);
    assert!(cartoon.set_option("style", "rocket").is_ok());
    let mut spacefill = Form::new(FormKind::Spacefill);
    assert!(matches!(
        spacefill.set_option("style", "rocket"),
        Err(OptionError::Unknown { .. })
    ));
}

#[test]
fn a_numeric_control_rejects_a_non_positive_value() {
    let mut form = Form::new(FormKind::Spacefill);
    assert!(matches!(
        form.set_option("radius", "-1"),
        Err(OptionError::Value(_))
    ));
    assert!(matches!(
        form.set_option("radius", "wide"),
        Err(OptionError::Value(_))
    ));
    assert!(form.set_option("radius", "0.5").is_ok());
}

#[test]
fn a_choice_control_lists_its_words() {
    let style = FormKind::Surface
        .options()
        .iter()
        .find(|option| option.name == "style");
    assert!(matches!(
        style.map(|option| option.kind),
        Some(OptionKind::Choice(words)) if words.contains(&"mesh")
    ));
}

#[test]
fn a_form_with_controls_serializes_only_what_is_set() {
    let mut form = Form::new(FormKind::Cartoon);
    assert!(form.set_option("width", "2").is_ok());
    let json = serde_json::to_string(&form).ok();
    assert_eq!(json.as_deref(), Some(r#"{"form":"cartoon","width":2.0}"#));
    let back: Option<Form> = json.and_then(|json| serde_json::from_str(&json).ok());
    assert_eq!(back, Some(form));
}
