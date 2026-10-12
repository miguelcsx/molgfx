use super::*;
use molgfx_core::{AtomSelection, EntityId, EntityKind};
use molgfx_math::catmull_rom_position;

fn structure() -> molframe::Structure {
    let cif = "data_curve\nloop_\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n1 C CA GLY A 1 0 0 0\n2 C CA GLY A 2 2 2 0\n3 C CA GLY A 3 4 0 0\n4 C CA GLY A 4 6 1 1\n5 C CA GLY A 5 8 0 0\n";
    molframe::read_bytes(
        cif.as_bytes().to_vec(),
        Some("curve.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("polymer fixture")
    .0
}

#[test]
fn discrete_protein_solids_do_not_substitute_a_coil_for_nucleic_sugar_guides() {
    let source = molframe::read_bytes(
        b"ATOM      1  C4'  DA A   1      -3.000   0.000   0.000  1.00 10.00           C\nATOM      2  C4'  DA A   2       0.000   2.000   0.000  1.00 10.00           C\nATOM      3  C4'  DA A   3       3.000   0.000   0.000  1.00 10.00           C\nEND\n".to_vec(),
        Some("nucleic.pdb"), &molframe::ReadOptions::new(),
    ).expect("source").0;
    let mut mesh = RibbonMesh::default();
    for profile in [
        SplineProfile::Rocket,
        SplineProfile::Cartoon,
        SplineProfile::NewRibbons,
    ] {
        mesh.generate_structure(
            &source,
            &AtomSelection::All,
            &[],
            8.0,
            RibbonParams {
                profile,
                ..RibbonParams::default()
            },
        )
        .expect("geometry");
        assert_eq!(mesh.indices.is_empty(), profile == SplineProfile::Rocket);
    }
}

#[test]
fn one_selected_guide_keeps_both_half_intervals_and_the_original_spline_controls() {
    let structure = structure();
    let points = structure
        .coordinates()
        .iter()
        .copied()
        .map(Vec3::from_array)
        .collect::<Vec<_>>();
    let selection = AtomSelection::Range(2..3);
    let owner = EntityId::pack(EntityKind::Atom, 2)
        .expect("atom identity")
        .0;
    for profile in [
        SplineProfile::Trace,
        SplineProfile::Tube,
        SplineProfile::Cartoon,
        SplineProfile::NewRibbons,
    ] {
        for width in if matches!(profile, SplineProfile::Trace | SplineProfile::Tube) {
            vec![0.0, 0.8]
        } else {
            vec![0.8]
        } {
            let mut mesh = RibbonMesh::default();
            mesh.generate_structure(
                &structure,
                &selection,
                &[],
                8.0,
                RibbonParams {
                    profile,
                    width,
                    thickness: width,
                    ..RibbonParams::default()
                },
            )
            .expect("selected span");
            assert!(!mesh.indices.is_empty());
            assert!(mesh.vertices.iter().all(|vertex| vertex.entity_id == owner));
            assert_eq!(
                mesh.deformations.first().expect("first recipe").controls,
                [0, 1, 2, 3]
            );
            assert_eq!(
                mesh.deformations.last().expect("last recipe").controls,
                [1, 2, 3, 4]
            );
            if width > 0.0 {
                let first = mesh.samples.first().expect("first sample");
                let last = mesh.samples.last().expect("last sample");
                assert_eq!((first.segment, first.parameter), (1, 0.5));
                assert_eq!((last.segment, last.parameter), (2, 0.5));
                let expected = if profile == SplineProfile::Trace {
                    points[1].lerp(points[2], 0.5)
                } else {
                    catmull_rom_position([points[0], points[1], points[2], points[3]], 0.5)
                };
                assert!(first.position.distance(expected) < 1.0e-6);
            } else {
                let first = &mesh.vertices[0];
                let expected = if profile == SplineProfile::Trace {
                    points[1].lerp(points[2], 0.5)
                } else {
                    catmull_rom_position([points[0], points[1], points[2], points[3]], 0.5)
                };
                assert!(Vec3::from_array(first.position).distance(expected) < 1.0e-6);
            }
        }
    }
}

#[test]
fn disjoint_selected_guides_do_not_bridge_an_unselected_residue() {
    let structure = structure();
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(
        &structure,
        &AtomSelection::Sparse(vec![1, 3]),
        &[],
        8.0,
        RibbonParams {
            profile: SplineProfile::Tube,
            width: 0.8,
            thickness: 0.8,
            ..RibbonParams::default()
        },
    )
    .expect("two independent spans");
    assert_eq!(mesh.traces.ranges.len(), 2);
    let owners = [1, 3].map(|row| EntityId::pack(EntityKind::Atom, row).expect("identity").0);
    assert!(owners.iter().all(|owner| {
        mesh.vertices
            .iter()
            .any(|vertex| vertex.entity_id == *owner)
    }));
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| owners.contains(&vertex.entity_id))
    );
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let first = mesh.vertices[triangle[0] as usize].entity_id;
        assert!(
            triangle
                .iter()
                .all(|index| mesh.vertices[*index as usize].entity_id == first)
        );
    }
}

#[test]
fn a_reused_mesh_drops_contextual_visibility_when_switching_to_a_complete_cartoon() {
    let structure = structure();
    let mut reused = RibbonMesh::default();
    reused
        .generate_structure(
            &structure,
            &AtomSelection::Range(2..3),
            &[],
            8.0,
            RibbonParams {
                profile: SplineProfile::Tube,
                ..RibbonParams::default()
            },
        )
        .expect("selected tube");
    reused
        .generate_structure(
            &structure,
            &AtomSelection::All,
            &[],
            8.0,
            RibbonParams::default(),
        )
        .expect("complete cartoon");
    let mut fresh = RibbonMesh::default();
    fresh
        .generate_structure(
            &structure,
            &AtomSelection::All,
            &[],
            8.0,
            RibbonParams::default(),
        )
        .expect("fresh cartoon");
    assert_eq!(reused.vertices, fresh.vertices);
    assert_eq!(reused.indices, fresh.indices);
}
