use super::*;
use serde_json::json;

fn fixture(form: &str, script: Option<&Script>) -> Fixture {
    serde_json::from_value::<Fixture>(json!({
        "id": "case", "file": "missing.cif", "format": "mmcif", "sha256": "0",
        "license": "CC0-1.0", "license_url": "https://example.org", "selection": "all",
        "form": form,
        "camera": {"position":[0,0,1],"target":[0,0,0],"up":[0,1,0],"fov_y_degrees":45,"near":1,"far":2},
        "script": script,
    }))
    .expect("fixture parses")
}

fn script(checklist: &[&str]) -> Script {
    Script {
        molgfx: Vec::new(),
        pymol: Vec::new(),
        molstar: Vec::new(),
        checklist: checklist.iter().map(|id| (*id).to_owned()).collect(),
        fit: None,
        video: None,
    }
}

fn message(fixture: &Fixture) -> String {
    fixture
        .verify(Path::new("/nonexistent"))
        .expect_err("verification fails")
        .to_string()
}

#[test]
fn a_script_case_without_a_script_is_rejected() {
    assert!(message(&fixture("script", None)).contains("has no script"));
}

#[test]
fn a_script_on_a_non_script_form_is_rejected() {
    assert!(message(&fixture("cartoon", Some(&script(&[])))).contains("form is not"));
}

#[test]
fn an_unknown_checklist_id_is_rejected_before_any_file_is_opened() {
    let case = fixture("script", Some(&script(&["GEN-1", "NOPE-9"])));
    assert!(message(&case).contains("unknown checklist id NOPE-9"));
}

#[test]
fn known_checklist_ids_pass_the_script_checks_and_reach_the_file() {
    let case = fixture("script", Some(&script(&["GEN-1", "SS-4", "VID-2"])));
    // The remaining failure is the absent file, not the script.
    assert!(!message(&case).contains("checklist"));
}
