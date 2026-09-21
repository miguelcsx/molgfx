// The index-to-float casts below build fixture coordinates from a loop
// counter whose range is a fixed literal in this file; nothing truncates.

use super::*;
use molgfx_core::{
    AtomProperty, AtomPropertyMeaning, ColorScheme, PropertyAppearance, RepresentationKind,
    ScalarFieldSemantics, Scene, SurfaceKind,
};
use molgfx_math::Rgba8;
use std::sync::Arc;

fn scene_table() -> (Scene, molgfx_core::RepresentationHandle) {
    let structure = fixture_structure();
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(e) => panic!("fixture scene builds: {e}"),
    };
    let sel = scene.add_selection(AtomSelection::All);
    let rep = match scene.represent(sel, RepresentationKind::Spacefill) {
        Ok(rep) => rep,
        Err(e) => panic!("spacefill applies: {e}"),
    };
    (scene, rep)
}

pub(crate) fn fixture_structure() -> molframe::Structure {
    let cif = "\
data_test
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.auth_seq_id
_atom_site.auth_asym_id
_atom_site.pdbx_PDB_model_num
ATOM 1 N N  . GLY A 1 1 0.0 0.0 0.0 1.00 10.0 1 A 1
ATOM 2 C CA . GLY A 1 1 1.5 0.0 0.0 1.00 10.0 1 A 1
ATOM 3 O O  . GLY A 1 1 3.0 1.0 0.0 1.00 10.0 1 A 1
loop_
_struct_conn.id
_struct_conn.conn_type_id
_struct_conn.ptnr1_label_asym_id
_struct_conn.ptnr1_label_seq_id
_struct_conn.ptnr1_label_comp_id
_struct_conn.ptnr1_label_atom_id
_struct_conn.ptnr2_label_asym_id
_struct_conn.ptnr2_label_seq_id
_struct_conn.ptnr2_label_comp_id
_struct_conn.ptnr2_label_atom_id
_struct_conn.pdbx_value_order
1 covale A 1 GLY N A 1 GLY CA SING
2 covale A 1 GLY CA A 1 GLY O AROM
";
    let options = molframe::ReadOptions::new();
    match molframe::read_bytes(cif.as_bytes().to_vec(), Some("t.cif"), &options) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("fixture parses: {diagnostics:?}"),
    }
}

#[test]
fn packing_all_atoms_produces_one_record_per_atom_in_order() {
    let (scene, rep_handle) = scene_table();
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(rep) = scene.representation(rep_handle) else {
        panic!("representation resolves")
    };
    let mut out = Vec::new();
    pack_atoms(table, rep, &AtomSelection::All, &mut out).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(out.len(), 3);
    // Records preserve atom order and name their source row; the shader gathers
    // the position from the coordinate column through that identity.
    assert_eq!(out[1].entity_id.unpack(), Some((EntityKind::Atom, 1)));
    let expected = [1.5f32, 0.0, 0.0];
    let position = table.coords().slice().get(1).copied().unwrap_or_default();
    for (got, want) in position.iter().zip(expected) {
        assert!((got - want).abs() < f32::EPSILON);
    }
    // Spacefill draws full van der Waals radii.
    assert!(
        (out[0].radius - 1.55).abs() < 1e-6,
        "nitrogen radius unscaled"
    );
}

#[test]
fn ball_and_stick_scales_radii_down() {
    let (mut scene, _) = scene_table();
    let sel = scene.add_selection(AtomSelection::All);
    let Ok(bs) = scene.represent(sel, RepresentationKind::BallAndStick) else {
        panic!("ball-and-stick applies")
    };
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(rep) = scene.representation(bs) else {
        panic!("representation resolves")
    };
    let mut out = Vec::new();
    pack_atoms(table, rep, &AtomSelection::All, &mut out).unwrap_or_else(|error| panic!("{error}"));
    assert!(out[0].radius < 1.0, "ball radii are a fraction of vdW");
}

#[test]
fn negative_radius_scale_cannot_create_inverted_gpu_spheres() {
    let (mut scene, rep_handle) = scene_table();
    let Some(representation) = scene.representation_mut(rep_handle) else {
        panic!("representation resolves")
    };
    representation.params.radius_scale = -4.0;
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(representation) = scene.representation(rep_handle) else {
        panic!("representation resolves")
    };
    let mut atoms = Vec::new();
    pack_atoms(table, representation, &AtomSelection::All, &mut atoms)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(atoms.iter().all(|atom| atom.radius == 0.0));
}

#[test]
fn exact_sas_impostors_pack_the_probe_once() {
    let (mut scene, rep_handle) = scene_table();
    let Some(representation) = scene.representation_mut(rep_handle) else {
        panic!("representation resolves")
    };
    representation.kind = RepresentationKind::Surface;
    representation.params.surface_kind = SurfaceKind::SolventAccessible;
    let probe = representation.params.probe_radius;
    let mut atoms = Vec::new();
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(representation) = scene.representation(rep_handle) else {
        panic!("representation resolves")
    };
    let expected = table.radius().values()[0] * representation.params.radius_scale + probe;
    pack_atoms(table, representation, &AtomSelection::All, &mut atoms)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(atoms[0].radius.to_bits(), expected.to_bits());
}

#[test]
fn representation_opacity_is_packed_once_for_transparent_shaders() {
    let (mut scene, rep_handle) = scene_table();
    let Some(representation) = scene.representation_mut(rep_handle) else {
        panic!("representation resolves")
    };
    representation.material.opacity = 0.4;
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(representation) = scene.representation(rep_handle) else {
        panic!("representation resolves")
    };
    let mut atoms = Vec::new();
    pack_atoms(table, representation, &AtomSelection::All, &mut atoms)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(atoms.iter().all(|atom| atom.color.a == 102));
}

#[test]
fn a_uniform_scheme_replaces_element_rgb_without_losing_material_opacity() {
    let (mut scene, rep_handle) = scene_table();
    let Some(representation) = scene.representation_mut(rep_handle) else {
        panic!("representation resolves")
    };
    representation.color = ColorScheme::Uniform(Rgba8::opaque(12, 34, 56));
    representation.material.opacity = 0.5;
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(representation) = scene.representation(rep_handle) else {
        panic!("representation resolves")
    };
    let mut atoms = Vec::new();
    pack_atoms(table, representation, &AtomSelection::All, &mut atoms)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        atoms
            .iter()
            .all(|atom| atom.color == Rgba8::new(12, 34, 56, 128))
    );
}

#[test]
fn caller_property_colors_are_reversible_and_preserve_missing_values() {
    let (mut scene, rep_handle) = scene_table();
    let Some((owner, _)) = scene.structures().next() else {
        panic!("structure exists")
    };
    let property = AtomProperty::new(
        owner,
        "confidence",
        Arc::from([0.0, 0.5, f32::NAN]),
        AtomPropertyMeaning::Confidence,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("property validates: {error}"));
    let property_handle = scene
        .add_atom_property(property)
        .unwrap_or_else(|error| panic!("property attaches: {error}"));
    let Some(property) = scene.atom_property(property_handle) else {
        panic!("property resolves")
    };
    let scheme = ColorScheme::property(property_handle, property);
    let ColorScheme::ByProperty { ramp, missing, .. } = scheme else {
        panic!("property constructor selects a property scheme")
    };
    let Some(representation) = scene.representation_mut(rep_handle) else {
        panic!("representation resolves")
    };
    representation.color = scheme;
    let Some((_, placed)) = scene.structures().next() else {
        panic!("structure exists")
    };
    let Some(representation) = scene.representation(rep_handle) else {
        panic!("representation resolves")
    };
    let property = scene.atom_property(property_handle);
    let mut atoms = Vec::new();
    pack_atoms_with_hierarchy(
        &placed.atoms,
        &placed.hierarchy,
        placed.secondary_structure.values(),
        property,
        representation,
        &AtomSelection::All,
        &mut atoms,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let colors = ramp.colors();
    assert_eq!(atoms[0].color, colors[0]);
    assert_eq!(atoms[1].color, colors[2]);
    assert_eq!(atoms[2].color, missing);
}

#[test]
fn confidence_appearance_packs_reversible_opacity_and_analytic_softness() {
    let (mut scene, rep_handle) = scene_table();
    let Some((owner, _)) = scene.structures().next() else {
        panic!("structure exists")
    };
    let property = AtomProperty::new(
        owner,
        "confidence",
        Arc::from([0.0, 1.0, f32::NAN]),
        AtomPropertyMeaning::Confidence,
        ScalarFieldSemantics::UncalibratedRank,
    )
    .unwrap_or_else(|error| panic!("property validates: {error}"));
    let property_handle = scene
        .add_atom_property(property)
        .unwrap_or_else(|error| panic!("property attaches: {error}"));
    let appearance = PropertyAppearance::confidence(property_handle, [0.0, 1.0])
        .unwrap_or_else(|error| panic!("appearance validates: {error}"));
    let Some(representation) = scene.representation_mut(rep_handle) else {
        panic!("representation resolves")
    };
    representation.appearance = Some(appearance);
    let Some((_, placed)) = scene.structures().next() else {
        panic!("structure exists")
    };
    let Some(representation) = scene.representation(rep_handle) else {
        panic!("representation resolves")
    };
    let property = scene.atom_property(property_handle);
    let mut atoms = Vec::new();
    pack_atoms_with_properties(
        &placed.atoms,
        &placed.hierarchy,
        placed.secondary_structure.values(),
        PropertyColumns {
            color: None,
            appearance: property,
        },
        representation,
        &AtomSelection::All,
        &mut atoms,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert!(atoms[0].color.a < atoms[1].color.a);
    assert!(atoms[0].semantic >> 24 > atoms[1].semantic >> 24);
    assert!(atoms[2].color.a < atoms[0].color.a);
    assert_ne!(atoms[2].semantic >> 24, 0);
}

#[test]
fn a_partial_selection_packs_only_its_rows() {
    let (scene, rep_handle) = scene_table();
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(rep) = scene.representation(rep_handle) else {
        panic!("representation resolves")
    };
    let mut out = Vec::new();
    pack_atoms(table, rep, &AtomSelection::Sparse(vec![0, 2]), &mut out)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(out.len(), 2);
    let (Some((_, i0)), Some((_, i2))) = (out[0].entity_id.unpack(), out[1].entity_id.unpack())
    else {
        panic!("entity ids unpack")
    };
    assert_eq!((i0, i2), (0, 2), "entity ids keep original row indices");
}

#[test]
fn licorice_uses_the_bond_radius_for_atom_junctions() {
    let structure = fixture_structure();
    let mut scene = match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene builds: {error}"),
    };
    let selection = scene.add_selection(AtomSelection::All);
    let representation = match scene.represent(selection, RepresentationKind::Licorice) {
        Ok(handle) => handle,
        Err(error) => panic!("licorice applies: {error}"),
    };
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(representation) = scene.representation(representation) else {
        panic!("representation resolves")
    };
    let mut atoms = Vec::new();
    pack_atoms(table, representation, &AtomSelection::All, &mut atoms)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        atoms
            .iter()
            .all(|atom| (atom.radius - representation.params.bond_radius).abs() < f32::EPSILON)
    );
}

#[test]
fn repacking_reuses_the_scratch_allocation() {
    let (scene, rep_handle) = scene_table();
    let Some(table) = scene.first_atoms() else {
        panic!("scene has atoms")
    };
    let Some(rep) = scene.representation(rep_handle) else {
        panic!("representation resolves")
    };
    let mut out = Vec::new();
    pack_atoms(table, rep, &AtomSelection::All, &mut out).unwrap_or_else(|error| panic!("{error}"));
    let capacity = out.capacity();
    let pointer = out.as_ptr();
    pack_atoms(table, rep, &AtomSelection::All, &mut out).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(out.capacity(), capacity);
    assert_eq!(
        out.as_ptr(),
        pointer,
        "steady-state repack allocates nothing"
    );
}

#[test]
fn a_bead_encloses_its_residue_and_keeps_one_sphere_per_residue() {
    use molgfx_core::{AtomSelection, RepresentationTarget, Scene};

    let structure = fixture_structure();
    let mut scene = Scene::new();
    let Ok(handle) = scene.add_structure(&structure) else {
        panic!("fixture places")
    };
    let selection = AtomSelection::All;
    let selection_handle = scene.add_selection(AtomSelection::All);
    let mut representation = Representation::new(
        RepresentationTarget::Selection(selection_handle),
        RepresentationKind::Beads,
    );
    representation.params.radius_scale = 1.0;
    let Some(placed) = scene.structure(handle) else {
        panic!("placed structure resolves")
    };

    let mut beads = Vec::new();
    pack_residue_beads(
        &placed.atoms,
        &placed.hierarchy,
        placed.secondary_structure.values(),
        None,
        &representation,
        &selection,
        &mut beads,
    )
    .unwrap_or_else(|error| panic!("{error}"));

    let residues = placed.atoms.residue().values();
    let distinct = {
        let mut seen: Vec<u32> = Vec::new();
        for residue in residues {
            if !seen.contains(residue) {
                seen.push(*residue);
            }
        }
        seen.len()
    };
    assert_eq!(beads.len(), distinct, "one bead per residue");

    // Each bead answers for its own residue: it names the residue's first atom,
    // and its radius must enclose every one of that residue's atoms, measured
    // from the first selected atom whose coordinate the shader gathers.
    let coords = placed.atoms.coords().slice();
    let radii = placed.atoms.radius().values();
    for bead in &beads {
        let Some((kind, first)) = bead.entity_id.unpack() else {
            panic!("bead names a source row");
        };
        assert_eq!(kind, EntityKind::Atom);
        let Some(residue) = residues.get(first as usize) else {
            panic!("bead source row is a real atom");
        };
        let members: Vec<usize> = residues
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate == &residue)
            .map(|(index, _)| index)
            .collect();
        let centre = coords
            .get(first as usize)
            .map_or(molgfx_math::Vec3::ZERO, |value| {
                molgfx_math::Vec3::from_array(*value)
            });
        for member in members {
            let Some(point) = coords.get(member).copied() else {
                continue;
            };
            let point = molgfx_math::Vec3::from_array(point);
            let extent = radii.get(member).copied().unwrap_or_default();
            assert!(
                centre.distance(point) + extent <= bead.radius + 1.0e-3,
                "residue {residue} atom {member} lies inside its bead"
            );
        }
    }
}
