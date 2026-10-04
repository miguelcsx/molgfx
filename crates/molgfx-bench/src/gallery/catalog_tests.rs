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
#[test]
fn unsupported_references_require_a_reason_and_cannot_hide_native_failures() {
    let mut case = fixture("cartoon", None);
    case.omissions.insert("pymol-ray".into(), " ".into());
    assert!(message(&case).contains("requires a reason"));
    case.omissions.clear();
    case.omissions
        .insert("molgfx-converged".into(), "not supported".into());
    assert!(message(&case).contains("cannot omit MolGFX"));
}

#[test]
fn phase_one_secondary_cases_retain_all_four_review_items_and_reference_recipes() {
    let catalog: Catalog =
        serde_json::from_str(include_str!("../../parity/gallery.json")).expect("gallery parses");
    for id in ["P1-ss-palette-1AON", "P1-ss-palette-7QPD"] {
        let fixture = catalog
            .fixtures
            .iter()
            .find(|fixture| fixture.id == id)
            .expect("secondary gallery case exists");
        let script = fixture.script.as_ref().expect("case has command script");
        for item in ["SS-1", "SS-2", "SS-3", "SS-4"] {
            assert!(script.checklist.iter().any(|entry| entry == item));
        }
        assert_eq!(
            script.molgfx,
            [
                "show cartoon as main, protein",
                "color secondary_structure, @main"
            ]
        );
        assert_eq!(script.molstar[0]["params"]["color"], "secondary-structure");
        assert!(!script.pymol.is_empty());
    }
    for recipe in [
        "molgfx-converged",
        "molgfx-interactive",
        "pymol-ray",
        "molstar-imagepass",
    ] {
        assert!(catalog.recipes.iter().any(|entry| entry == recipe));
    }
}

#[test]
fn the_segmentation_gallery_case_has_two_independent_maps_and_both_review_items() {
    let catalog: Catalog =
        serde_json::from_str(include_str!("../../parity/gallery.json")).expect("gallery parses");
    let case = catalog
        .fixtures
        .iter()
        .find(|case| case.id == "P2-segmentation-two-maps")
        .expect("segmentation case exists");
    let script = case.script.as_ref().expect("case has review items");
    assert_eq!(script.checklist, ["SEG-1", "SEG-2"]);
    assert!(
        script.fit.is_none(),
        "categorical scene has an explicit camera"
    );
    let maps = case.details["segmentations"].as_array().expect("two maps");
    assert_eq!(maps.len(), 2);
    assert_eq!(maps[0]["styles"][0]["label"], maps[1]["styles"][0]["label"]);
    assert_ne!(
        maps[0]["styles"][0]["color_rgb"],
        maps[1]["styles"][0]["color_rgb"]
    );
    assert_ne!(maps[0]["translation"], maps[1]["translation"]);
    assert_eq!(maps[1]["styles"][1]["visible"], false);
    assert_eq!(
        case.details["segmentation_thresholds"],
        json!([0.05, 0.2, 0.5])
    );
}
