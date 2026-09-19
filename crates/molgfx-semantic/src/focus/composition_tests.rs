use super::*;
use molgfx_core::AtomSelection;

const SOURCE: &str = "data_focus\n\
loop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n\
_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n\
_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n\
ATOM 1 C CA ALA A 1 0 0 0\nATOM 2 O O ALA A 1 0.8 0 0\n\
HETATM 3 C C1 LIG A 2 2.0 0 0\nHETATM 4 N N1 LIG A 2 2.8 0 0\n\
ATOM 5 C CA GLY A 3 5.0 0 0\nATOM 6 O O GLY A 3 5.8 0 0\n\
ATOM 7 C CA SER A 4 14.0 0 0\nATOM 8 O O SER A 4 14.8 0 0\n";

fn scene() -> Scene {
    let options = molframe::ReadOptions::new();
    let (structure, _) =
        match molframe::read_bytes(SOURCE.as_bytes().to_vec(), Some("focus.cif"), &options) {
            Ok(structure) => structure,
            Err(diagnostics) => panic!("focus fixture parses: {diagnostics:?}"),
        };
    match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("focus scene builds: {error}"),
    }
}

#[test]
fn default_focus_bands_match_interaction_and_context_shells() {
    let bands = DistanceBands::default();
    assert_eq!(bands.classify(3.99), FocusBand::Near);
    assert_eq!(bands.classify(4.0), FocusBand::Mid);
    assert_eq!(bands.classify(10.0), FocusBand::Far);
    assert_eq!(bands.classify(f32::NAN), FocusBand::Far);
}

#[test]
fn invalid_thresholds_are_typed_errors() {
    assert_eq!(
        DistanceBands::new(5.0, 4.0),
        Err(FocusError::InvalidDistances)
    );
}

#[test]
fn focus_composes_disjoint_distance_bands_and_an_atom_bounded_pocket() {
    let mut scene = scene();
    let ligand = scene.add_selection(AtomSelection::Sparse(vec![2, 3]));
    let view = match scene.focus(ligand) {
        Ok(view) => view,
        Err(error) => panic!("focus composition succeeds: {error}"),
    };
    let selected = |handle| {
        scene
            .selection(handle)
            .map(|selection| selection.to_bitmap(8).iter().collect::<Vec<_>>())
    };
    assert_eq!(selected(view.focus), Some(vec![2, 3]));
    assert_eq!(selected(view.pocket), Some(vec![0, 1, 4, 5]));
    assert_eq!(selected(view.near), Some(vec![0, 1, 4, 5]));
    assert_eq!(selected(view.mid), Some(Vec::new()));
    assert_eq!(selected(view.context), Some(vec![6, 7]));
    assert_eq!(selected(view.solvent), Some(Vec::new()));
    let Some(pocket) = scene.representation(view.pocket_representation) else {
        panic!("pocket representation resolves")
    };
    assert_eq!(pocket.kind, RepresentationKind::Surface);
    assert_eq!(pocket.params.surface_kind, SurfaceKind::SolventExcluded);
    assert_eq!(pocket.params.surface_style, SurfaceStyle::Solid);
    let Some(context) = scene.representation(view.context_representation) else {
        panic!("context representation resolves")
    };
    assert_eq!(context.kind, RepresentationKind::Tube);
    let Some(solvent) = scene.representation(view.solvent_representation) else {
        panic!("solvent representation resolves")
    };
    assert_eq!(solvent.kind, RepresentationKind::Spacefill);
}

#[test]
fn structure_extent_reuses_one_continuous_non_solvent_boundary() {
    let mut scene = scene();
    let ligand = scene.add_selection(AtomSelection::Sparse(vec![2, 3]));
    let view = match scene.focus_with(
        ligand,
        FocusStyle {
            surface_extent: FocusSurfaceExtent::Structure,
            ..FocusStyle::default()
        },
    ) {
        Ok(view) => view,
        Err(error) => panic!("continuous focus composition succeeds: {error}"),
    };
    let Some(boundary) = scene.selection(view.pocket) else {
        panic!("boundary selection resolves")
    };
    assert_eq!(
        boundary.to_bitmap(8).iter().collect::<Vec<_>>(),
        vec![0, 1, 4, 5, 6, 7]
    );
    assert_eq!(
        scene
            .representations()
            .filter(|(_, representation)| representation.kind == RepresentationKind::Surface)
            .count(),
        1
    );
}
