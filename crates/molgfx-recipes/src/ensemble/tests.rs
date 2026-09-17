use super::*;
use molgfx_core::{Scene, StructureHandle};

fn structure() -> pdbiox::Structure {
    let cif = "data_ensemble\n\
loop_\n\
_atom_site.group_PDB\n\
_atom_site.id\n\
_atom_site.type_symbol\n\
_atom_site.label_atom_id\n\
_atom_site.label_alt_id\n\
_atom_site.label_comp_id\n\
_atom_site.label_asym_id\n\
_atom_site.label_entity_id\n\
_atom_site.label_seq_id\n\
_atom_site.Cartn_x\n\
_atom_site.Cartn_y\n\
_atom_site.Cartn_z\n\
_atom_site.occupancy\n\
_atom_site.B_iso_or_equiv\n\
_atom_site.auth_seq_id\n\
_atom_site.auth_asym_id\n\
_atom_site.pdbx_PDB_model_num\n\
ATOM 1 C CA . GLY A 1 1 0 0 0 1 10 1 A 1\n";
    match pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("ensemble.cif"),
        &pdbiox::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("fixture parses: {diagnostics:?}"),
    }
}

fn ensemble_scene() -> (Scene, [StructureHandle; 2]) {
    let structure = structure();
    let mut scene = Scene::new();
    let first = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    let second = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    (scene, [first, second])
}

#[test]
fn overlay_is_structure_scoped_weighted_and_cycleable() {
    let (mut scene, members) = ensemble_scene();
    let view = scene
        .overlay_ensemble(
            &members,
            &[0.8, 0.2],
            "caller:weighted-poses",
            EnsembleStyle {
                representation: RepresentationKind::Spacefill,
                ..EnsembleStyle::default()
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(view.representations.len(), 2);
    assert_eq!(view.dominant, 0);
    let opacity = view
        .representations
        .iter()
        .filter_map(|handle| scene.representation(*handle))
        .filter_map(|representation| representation.visual.as_ref())
        .map(|visual| {
            visual
                .evaluate(molgfx_core::VisualInputs::default())
                .opacity
        })
        .collect::<Vec<_>>();
    assert!((opacity[0] - 1.0).abs() < f32::EPSILON);
    assert!(opacity[1] < opacity[0]);

    view.show_member(&mut scene, 1)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!scene
        .representation(view.representations[0])
        .is_some_and(|value| value.visible));
    assert!(scene
        .representation(view.representations[1])
        .is_some_and(|value| value.visible));
    view.show_overlay(&mut scene);
    assert!(view.representations.iter().all(|handle| {
        scene
            .representation(*handle)
            .is_some_and(|value| value.visible)
    }));
}

#[test]
fn invalid_overlay_style_is_a_typed_error() {
    let (mut scene, members) = ensemble_scene();
    let result = scene.overlay_ensemble(
        &members,
        &[0.8, 0.2],
        "caller:weighted-poses",
        EnsembleStyle {
            alternate_opacity: 0.2,
            minimum_opacity: 0.5,
            ..EnsembleStyle::default()
        },
    );
    assert!(matches!(result, Err(CoreError::InvalidEnsemble { .. })));
}
