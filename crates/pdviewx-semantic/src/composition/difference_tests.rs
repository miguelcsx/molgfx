use super::*;
use pdviewx_math::{Mat4, Vec3};

fn structure() -> pdbiox::Structure {
    let cif = "data_difference\n\
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
        Some("difference.cif"),
        &pdbiox::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("fixture parses: {diagnostics:?}"),
    }
}

fn placed_pair() -> (Scene, [StructureHandle; 2]) {
    let structure = structure();
    let mut scene = Scene::new();
    let left = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    let right = scene
        .add_structure(&structure)
        .unwrap_or_else(|error| panic!("{error}"));
    let Some(placed) = scene.structure_mut(right) else {
        panic!("right placement resolves")
    };
    placed.model_to_world = Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0));
    (scene, [left, right])
}

#[test]
fn caller_correspondence_drives_reversible_world_space_difference_views() {
    let (mut scene, structures) = placed_pair();
    let view = scene
        .render_difference(
            structures,
            &[AtomCorrespondence { left: 0, right: 0 }],
            "caller:alignment/v1",
            DifferenceStyle {
                representation: RepresentationKind::Spacefill,
                ..DifferenceStyle::default()
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));
    assert!((view.maximum_displacement - 2.0).abs() < 1.0e-6);
    for (side, property) in view.properties.into_iter().enumerate() {
        let property = scene
            .atom_property(property)
            .unwrap_or_else(|| panic!("side {side} property resolves"));
        assert!((property.values()[0] - 2.0).abs() < 1.0e-6);
        let representation = scene
            .representation(view.representations[side])
            .unwrap_or_else(|| panic!("side {side} representation resolves"));
        assert!(representation.appearance.is_some());
        assert!(
            scene
                .selection_for(view.selections[side], structures[side])
                .is_some()
        );
    }
}

#[test]
fn repeated_correspondence_is_rejected_before_composition() {
    let (mut scene, structures) = placed_pair();
    let repeated = scene.render_difference(
        structures,
        &[
            AtomCorrespondence { left: 0, right: 0 },
            AtomCorrespondence { left: 0, right: 0 },
        ],
        "caller",
        DifferenceStyle::default(),
    );
    assert!(matches!(repeated, Err(CoreError::InvalidDifference { .. })));
    assert_eq!(scene.representation_count(), 0);
}
