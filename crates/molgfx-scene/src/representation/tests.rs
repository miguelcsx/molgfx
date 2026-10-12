use super::*;
use crate::{Scene, rep};

#[test]
fn zero_radius_curve_specs_lower_and_round_trip_without_an_epsilon_cylinder() {
    for spec in [
        RepresentationSpec::from(rep::trace("all").radius(0.0)),
        RepresentationSpec::from(rep::tube("all").radius(0.0)),
    ] {
        let spec = spec.structure(crate::StructureId::new(0));
        spec.validate().expect("pixel stroke");
        let (native, _) = spec
            .native(crate::spec::lowering::Lowering {
                surfaces: None,
                properties: &std::collections::BTreeMap::new(),
                channels: &[],
            })
            .expect("lower");
        let text = serde_json::to_string(&spec).expect("serialize");
        assert_eq!(
            serde_json::from_str::<RepresentationSpec>(&text).expect("restore"),
            spec
        );
        let mut scene = molgfx_core::Scene::from_structure(&structure()).expect("scene");
        let selection = scene.add_selection(molgfx_core::AtomSelection::All);
        let id = scene.represent(selection, native).expect("native curve");
        assert_eq!(
            scene
                .representation(id)
                .expect("curve")
                .params
                .tube_radius
                .to_bits(),
            0.0f32.to_bits()
        );
    }
}

fn structure() -> molframe::Structure {
    const PDB: &str =
        "ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n";
    let result = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("one.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    structure
}

#[test]
fn every_representation_form_round_trips_through_its_tagged_shape() {
    let specifications = vec![
        rep::cartoon("all").into(),
        rep::ribbons("all").edge_radius(0.0).into(),
        rep::new_ribbons("all").into(),
        rep::bonds("all").into(),
        rep::dynamic_bonds("all").into(),
        rep::polyhedra("all").into(),
        rep::solvent("all").into(),
        rep::paper_chain("all").into(),
        rep::backbone("all").into(),
        rep::trace("all").into(),
        rep::tube("all").into(),
        rep::putty("all").into(),
        rep::ball_and_stick("all").into(),
        rep::spacefill("all").into(),
        rep::licorice("all").into(),
        rep::lines("all").into(),
        rep::points("all").into(),
        rep::dots("all").into(),
        rep::surface("all").into(),
        rep::quick_surf("all").into(),
        rep::nucleic_acid("all").into(),
        rep::bases("all").into(),
        rep::base_pairs("all").into(),
        rep::glycan("all").into(),
    ];
    for specification in specifications {
        let json = serde_json::to_string(&specification)
            .unwrap_or_else(|error| panic!("specification must serialize: {error}"));
        let decoded: RepresentationSpec = serde_json::from_str(&json)
            .unwrap_or_else(|error| panic!("specification must deserialize: {error}"));
        assert_eq!(decoded, specification);
        assert!(json.contains("\"common\""));
        assert!(json.contains("\"kind\""));
    }
}

#[test]
fn the_superseded_optional_field_wire_shape_is_rejected() {
    let mut scene = Scene::from_structure(&structure())
        .unwrap_or_else(|error| panic!("scene must resolve: {error}"));
    let id = scene
        .add(rep::surface("all"))
        .unwrap_or_else(|error| panic!("surface must resolve: {error}"));
    let mut value = serde_json::to_value(scene.spec())
        .unwrap_or_else(|error| panic!("scene must serialize: {error}"));
    let Some(representations) = value
        .get_mut("representations")
        .and_then(serde_json::Value::as_object_mut)
    else {
        panic!("representations must be an object")
    };
    let _ = representations.insert(
        id.get().to_string(),
        serde_json::json!({
            "structure": 1,
            "target": "all",
            "form": "surface",
            "color": { "kind": "uniform", "color": [255, 255, 255, 255] },
            "opacity": 1.0,
            "probe_radius": 1.4,
            "visible": true
        }),
    );
    let source = serde_json::to_string(&value)
        .unwrap_or_else(|error| panic!("malformed fixture must serialize: {error}"));
    assert!(crate::SceneSpec::from_json(&source).is_err());
}

#[test]
fn a_blob_surface_round_trips_and_names_its_soft_union_style() {
    let specification: RepresentationSpec = rep::surface("all")
        .kind(SurfaceKind::VanDerWaals)
        .style(SurfaceStyle::SoftUnion)
        .probe_radius(1.6)
        .isolevel(0.6)
        .blob_spread(0.0)
        .into();
    let json = serde_json::to_string(&specification)
        .unwrap_or_else(|error| panic!("specification must serialize: {error}"));
    assert!(json.contains("\"style\":\"soft_union\""), "{json}");
    // Zero is a meaningful spread: it requests the exact union, so the control
    // must survive the wire rather than being normalised away.
    assert!(json.contains("\"blob_spread\":0.0"), "{json}");
    let decoded: RepresentationSpec = serde_json::from_str(&json)
        .unwrap_or_else(|error| panic!("specification must deserialize: {error}"));
    assert_eq!(decoded, specification);
}

#[test]
fn a_form_control_only_reaches_its_own_form() {
    let spec: RepresentationSpec = rep::surface("all")
        .kind(SurfaceKind::Gaussian)
        .isolevel(0.75)
        .probe_radius(1.8)
        .into();
    let Ok(json) = serde_json::to_string(&spec) else {
        panic!("surface must serialize")
    };
    assert!(json.contains("\"kind\":\"surface\""), "{json}");
    assert!(json.contains("\"surface\":\"gaussian\""), "{json}");
    assert!(json.contains("\"isolevel\":0.75"), "{json}");

    // A cartoon carries no surface controls at all, in the builder or the wire.
    let cartoon: RepresentationSpec = rep::cartoon("all").width(2.0).into();
    let Ok(json) = serde_json::to_string(&cartoon) else {
        panic!("cartoon must serialize")
    };
    assert!(!json.contains("isolevel"), "{json}");
    assert!(!json.contains("probe_radius"), "{json}");
}

#[test]
fn cartoon_shape_is_validated_and_retained_by_portable_specifications() {
    let specification: RepresentationSpec = rep::new_cartoon("all")
        .aspect_ratio(3.0)
        .arrow_factor(2.0)
        .direction_wedges(true)
        .gaps(rep::GapStyle::Dashed)
        .helix_profile(rep::CartoonProfile::Rounded)
        .nucleic_profile(rep::CartoonProfile::Square)
        .into();
    let json = serde_json::to_string(&specification).expect("portable shape");
    let restored: RepresentationSpec = serde_json::from_str(&json).expect("shape restored");
    assert_eq!(restored, specification);
    assert!(restored.validate_values().is_ok());
    for (aspect, arrow) in [
        (0.0, 1.0),
        (5.0, -1.0),
        (f32::NAN, 1.0),
        (5.0, f32::INFINITY),
    ] {
        let invalid: RepresentationSpec = rep::cartoon("all")
            .aspect_ratio(aspect)
            .arrow_factor(arrow)
            .into();
        assert!(invalid.validate_values().is_err());
    }
}

#[test]
fn unsupported_soft_union_fields_are_rejected_instead_of_ignoring_the_style() {
    for kind in [SurfaceKind::Gaussian, SurfaceKind::SolventExcluded] {
        let specification: RepresentationSpec = rep::surface("all")
            .kind(kind)
            .style(SurfaceStyle::SoftUnion)
            .into();
        assert!(specification.validate_values().is_err());
    }
    for kind in [SurfaceKind::VanDerWaals, SurfaceKind::SolventAccessible] {
        let specification: RepresentationSpec = rep::surface("all")
            .kind(kind)
            .style(SurfaceStyle::SoftUnion)
            .blob_spread(5.0)
            .into();
        assert!(specification.validate_values().is_ok());
    }
}

#[test]
fn cartoon_controls_refuse_profiles_and_radii_that_the_chosen_geometry_cannot_use() {
    for specification in [
        RepresentationSpec::from(rep::cartoon("all").helix_profile(rep::CartoonProfile::Rounded)),
        RepresentationSpec::from(
            rep::cartoon("all").nucleic_profile(rep::CartoonProfile::Elliptical),
        ),
        RepresentationSpec::from(rep::new_cartoon("all").helix_radius(3.0)),
    ] {
        assert!(specification.validate_values().is_err());
    }
    assert!(
        RepresentationSpec::from(rep::cartoon("all").helix_radius(3.0))
            .validate_values()
            .is_ok()
    );
    assert!(
        RepresentationSpec::from(
            rep::cartoon("all")
                .style(rep::CartoonStyle::Ribbon)
                .helix_profile(rep::CartoonProfile::Rounded)
        )
        .validate_values()
        .is_ok()
    );
}

#[test]
fn gaussian_density_controls_lower_without_unit_changes_or_silent_clamps() {
    let specification: RepresentationSpec = rep::quick_surf("all")
        .structure(crate::StructureId::new(0))
        .radius(1.7)
        .sigma(0.03)
        .isolevel(0.00001)
        .grid_spacing(0.2)
        .cutoff(6.0)
        .into();
    specification.validate().expect("density recipe");
    let properties = std::collections::BTreeMap::new();
    let (config, _) = specification
        .native(crate::spec::lowering::Lowering {
            surfaces: None,
            properties: &properties,
            channels: &[],
        })
        .expect("density lowering");
    let mut scene = molgfx_core::Scene::new();
    let selection = scene.add_selection(molgfx_core::AtomSelection::All);
    let handle = scene.represent(selection, config).expect("native density");
    let native = scene.representation(handle).expect("density");
    assert_eq!(native.kind, molgfx_core::RepresentationKind::Surface);
    assert_eq!(
        native.params.surface_kind,
        molgfx_core::SurfaceKind::Gaussian
    );
    assert_eq!(native.params.radius_scale.to_bits(), 1.7f32.to_bits());
    assert_eq!(native.params.gaussian_sigma.to_bits(), 0.03f32.to_bits());
    assert_eq!(native.params.isolevel.to_bits(), 0.00001f32.to_bits());
    assert_eq!(
        native.params.surface_spacing.map(f32::to_bits),
        Some(0.2f32.to_bits())
    );
    assert_eq!(native.params.gaussian_cutoff.to_bits(), 6.0f32.to_bits());
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        for specification in [
            rep::quick_surf("all").radius(invalid),
            rep::quick_surf("all").sigma(invalid),
            rep::quick_surf("all").isolevel(invalid),
            rep::quick_surf("all").grid_spacing(invalid),
            rep::quick_surf("all").cutoff(invalid),
        ] {
            let specification: RepresentationSpec = specification.into();
            assert!(specification.validate().is_err());
        }
    }
}
