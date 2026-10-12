use super::{Form, FormKind, OptionError, OptionKind};

#[test]
fn cartoon_and_new_cartoon_lower_to_different_geometry_recipes() {
    for (kind, expected, width) in [
        (FormKind::Cartoon, "rocket", 2.0),
        (FormKind::NewCartoon, "ribbon", 1.2),
    ] {
        let specification = Form::new(kind).specification(
            "all".into(),
            super::Look {
                structure: molgfx_scene::StructureId::new(0),
                color: None,
                opacity: None,
            },
        );
        let json = serde_json::to_value(&specification).expect("specification");
        assert_eq!(json["form"]["style"], expected);
        let authored_width = json["form"]["width"].as_f64().expect("width");
        assert!((authored_width - width).abs() < 1.0e-6);
    }
    let mut classic = Form::new(FormKind::Cartoon);
    assert!(classic.set_option("helix_radius", "2.7").is_ok());
    let mut curved = Form::new(FormKind::NewCartoon);
    assert!(matches!(
        curved.set_option("helix_radius", "2.7"),
        Err(OptionError::Unknown { .. })
    ));
}

#[test]
fn trace_and_tube_accept_zero_radius_strokes_and_reject_negative_or_nonfinite_radii() {
    for kind in [FormKind::Trace, FormKind::Tube] {
        let mut form = Form::new(kind);
        assert!(form.set_option("radius", "0").is_ok());
        for invalid in ["-1", "NaN", "inf"] {
            assert!(form.set_option("radius", invalid).is_err());
        }
    }
}

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

#[test]
fn the_polymer_and_dot_forms_accept_only_their_declared_controls() {
    let mut putty = Form::new(FormKind::Putty);
    assert!(putty.set_option("domain_min", "0").is_ok());
    assert!(putty.set_option("domain_max", "50").is_ok());
    assert!(putty.set_option("radius_min", "0.2").is_ok());
    assert!(putty.set_option("radius_max", "2").is_ok());
    assert!(matches!(
        putty.set_option("width", "1"),
        Err(OptionError::Unknown { .. })
    ));
    let mut dots = Form::new(FormKind::Dots);
    assert!(dots.set_option("size", "3").is_ok());
    assert!(matches!(
        dots.set_option("radius", "1"),
        Err(OptionError::Unknown { .. })
    ));
}

#[test]
fn analytical_methods_require_a_real_content_source_and_expose_only_their_scientific_controls() {
    let hash = "ab".repeat(32);
    for kind in [FormKind::Surf, FormKind::Msms] {
        let mut form = Form::new(kind);
        assert!(form.set_option("source", "not-a-content-hash").is_err());
        form.set_option("source", &hash).expect("content source");
        form.set_option("probe_radius", "1.8").expect("probe");
        form.set_option("wire", "true").expect("wire");
        assert!(form.controls().contains(&format!("source={hash}")));
        let spec = form.specification(
            "all".into(),
            super::Look {
                structure: molgfx_scene::StructureId::new(0),
                color: None,
                opacity: None,
            },
        );
        assert_eq!(spec.form_name(), kind.name());
        let encoded = serde_json::to_string(&form).expect("serialize");
        assert_eq!(
            serde_json::from_str::<Form>(&encoded).expect("restore"),
            form
        );
    }
    assert!(
        Form::new(FormKind::Surf)
            .set_option("density", "1.0")
            .is_err()
    );
    assert!(
        Form::new(FormKind::Msms)
            .set_option("density", "1.0")
            .is_ok()
    );
}
