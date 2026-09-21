use super::{NodeSupport, classify};

#[test]
fn every_official_v1_node_kind_is_classified() {
    for kind in [
        "root",
        "download",
        "parse",
        "coordinates",
        "structure",
        "transform",
        "instance",
        "component",
        "component_from_uri",
        "component_from_source",
        "representation",
        "volume",
        "volume_representation",
        "color",
        "color_from_uri",
        "color_from_source",
        "clip",
        "opacity",
        "label",
        "label_from_uri",
        "label_from_source",
        "tooltip",
        "tooltip_from_uri",
        "tooltip_from_source",
        "focus",
        "primitives",
        "primitives_from_uri",
        "primitive",
        "camera",
        "canvas",
        "transition",
    ] {
        assert_ne!(classify(kind), NodeSupport::Unknown, "{kind}");
    }
}

#[test]
fn extension_nodes_are_not_mistaken_for_the_v1_schema() {
    assert_eq!(classify("org_molgfx_magic"), NodeSupport::Unknown);
}

#[test]
fn official_preset_selectors_map_without_heuristics() {
    let branched = serde_json::Value::String("branched".to_owned());
    let selector = super::molecular_selector(Some(&branched));
    assert!(matches!(selector, Ok(value) if value.as_ref() == "saccharide"));
    let structured = serde_json::json!({ "label_asym_id": "A" });
    assert!(super::molecular_selector(Some(&structured)).is_err());
}
