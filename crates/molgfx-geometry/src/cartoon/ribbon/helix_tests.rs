use super::*;
use crate::cartoon::ribbon::{RibbonParams, SplineProfile};
use molgfx_core::{AtomSelection, SecondaryStructure};
use molgfx_math::Vec3;
use std::fmt::Write as _;

fn long_helix() -> molframe::Structure {
    let mut cif = String::from(
        "data_helix\nloop_\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n",
    );
    for row in 0..16 {
        let x = f64::from(row) * 1.5;
        let y = f64::from(row).cos() * 2.0;
        let z = f64::from(row).sin() * 2.0;
        writeln!(cif, "{} C CA GLY A {} {x} {y} {z}", row + 1, row + 1).expect("fixture");
    }
    molframe::read_bytes(
        cif.into_bytes(),
        Some("helix.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("helix")
    .0
}

#[test]
fn isolated_helix_selections_keep_the_entire_fit_and_only_selected_atom_ownership() {
    let structure = long_helix();
    let generate = |selection| {
        let mut mesh = RibbonMesh::default();
        mesh.generate_structure(
            &structure,
            &selection,
            &[SecondaryStructure::AlphaHelix; 16],
            8.0,
            RibbonParams {
                profile: SplineProfile::Rocket,
                ..RibbonParams::default()
            },
        )
        .expect("selected helix");
        mesh.prepare_helix_fits().expect("complete fit");
        mesh
    };
    let full = generate(AtomSelection::All);
    let selected = generate(AtomSelection::Sparse(vec![4, 11]));
    assert!(!selected.vertices.is_empty());
    assert!(selected.vertices.iter().all(|vertex| matches!(
        EntityId(vertex.entity_id).unpack(),
        Some((EntityKind::Atom, 4 | 11))
    )));
    assert_eq!(
        selected.fit_jobs[0], 1,
        "disjoint spans share one scientific fit"
    );
    assert_eq!(selected.fit_jobs[2], 16);
    assert_eq!(
        &selected.fit_jobs[selected.fit_jobs[1] as usize..],
        &full.fit_jobs[full.fit_jobs[1] as usize..]
    );
    assert_eq!(
        &selected.fit_jobs[selected.fit_jobs[1] as usize..],
        &(0..16).collect::<Vec<_>>()
    );
    assert!(selected.vertices.len() < full.vertices.len());
    for mesh in [&full, &selected] {
        let offset = mesh.fit_jobs[3];
        for recipe in mesh
            .deformations
            .iter()
            .filter(|recipe| recipe.parameter[3].to_bits() == 6)
        {
            assert_eq!(recipe.parameter[1].to_bits(), offset);
            assert_eq!(
                recipe.parameter[2].to_bits(),
                (recipe.controls[0].to_f32().expect("small guide index") - 7.5).to_bits()
            );
        }
    }
}

#[test]
fn two_guide_columns_are_circular_and_extend_half_a_residue_at_each_end() {
    let mut mesh = RibbonMesh::default();
    mesh.generate_styled(
        &[molgfx_math::Vec3::ZERO, molgfx_math::Vec3::X * 2.0],
        &[8, 9],
        &[SecondaryStructure::AlphaHelix; 2],
        RibbonParams {
            profile: SplineProfile::Rocket,
            width: 0.6,
            helix_radius: 1.2,
            thickness: 0.03,
            ..RibbonParams::default()
        },
    )
    .expect("two guide column");
    mesh.prepare_helix_fits().expect("exact two point fit");
    assert_eq!(mesh.fit_jobs[2], 2);
    assert_eq!(&mesh.fit_jobs[mesh.fit_jobs[1] as usize..], &[8, 9]);
    assert_eq!(mesh.vertices.len(), 98);
    assert_eq!(mesh.indices.len(), 288);
    for (vertex, recipe) in mesh.vertices.iter().zip(&mesh.deformations) {
        assert_eq!(recipe.parameter[3].to_bits(), 6);
        assert!(
            (vertex.position[0] + 1.0).abs() < 1e-6
                || (vertex.position[0] - 1.0).abs() < 1e-6
                || (vertex.position[0] - 3.0).abs() < 1e-6
        );
        let radius = vertex.position[1].hypot(vertex.position[2]);
        assert!(radius < 1e-6 || (radius - 1.2).abs() < 1e-6);
    }
}

#[test]
fn a_coil_selection_keeps_the_full_hidden_helix_fit_and_its_extended_boundary() {
    let structure = long_helix();
    let mut mesh = RibbonMesh::default();
    let mut styles = [SecondaryStructure::AlphaHelix; 16];
    styles[0] = SecondaryStructure::Coil;
    mesh.generate_structure(
        &structure,
        &AtomSelection::Sparse(vec![0]),
        &styles,
        8.0,
        RibbonParams {
            profile: SplineProfile::Rocket,
            ..RibbonParams::default()
        },
    )
    .expect("coil");
    mesh.prepare_helix_fits().expect("neighbor fit");
    assert_eq!(mesh.fit_jobs[2], 15);
    assert_eq!(
        &mesh.fit_jobs[mesh.fit_jobs[1] as usize..],
        &(1..16).collect::<Vec<_>>()
    );
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| EntityId(vertex.entity_id).unpack() == Some((EntityKind::Atom, 0)))
    );
    assert!(
        mesh.deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 14)
    );
    assert!(
        mesh.deformations
            .iter()
            .flat_map(|recipe| recipe.controls)
            .any(|offset| {
                let stencil = &mesh.traces.properties[offset as usize..];
                stencil[1].to_bits() == 1
                    && stencil[3].to_bits() == 3
                    && stencil[2].to_bits() == (-7.5f32).to_bits()
            })
    );
}

#[test]
fn direct_guide_regeneration_cannot_reuse_a_previous_structures_helix_rows() {
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(
        &long_helix(),
        &AtomSelection::All,
        &[SecondaryStructure::AlphaHelix; 16],
        8.0,
        RibbonParams {
            profile: SplineProfile::Rocket,
            ..RibbonParams::default()
        },
    )
    .expect("structure");
    mesh.prepare_helix_fits().expect("old fits");
    mesh.generate_styled(
        &[molgfx_math::Vec3::ZERO, molgfx_math::Vec3::Y],
        &[40, 42],
        &[SecondaryStructure::AlphaHelix; 2],
        RibbonParams {
            profile: SplineProfile::Rocket,
            ..RibbonParams::default()
        },
    )
    .expect("replacement");
    mesh.prepare_helix_fits().expect("replacement fit");
    assert_eq!(&mesh.fit_jobs[mesh.fit_jobs[1] as usize..], &[40, 42]);
}

#[test]
fn classic_helix_radius_and_sheet_width_change_only_their_own_secondary_solids() {
    let points = [Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0), Vec3::X * 2.0];
    let entities = [0, 1, 2];
    let generate = |state, width, radius| {
        let mut mesh = RibbonMesh::default();
        mesh.generate_styled(
            &points,
            &entities,
            &[state; 3],
            RibbonParams {
                profile: SplineProfile::Rocket,
                width,
                helix_radius: radius,
                ..RibbonParams::default()
            },
        )
        .expect("independent shape");
        mesh
    };
    let thin_sheet_parameter = generate(SecondaryStructure::AlphaHelix, 0.6, 1.2);
    let wide_sheet_parameter = generate(SecondaryStructure::AlphaHelix, 2.4, 1.2);
    assert_eq!(thin_sheet_parameter.vertices, wide_sheet_parameter.vertices);
    let small_helix_parameter = generate(SecondaryStructure::Strand, 2.0, 0.6);
    let large_helix_parameter = generate(SecondaryStructure::Strand, 2.0, 2.4);
    assert_eq!(
        small_helix_parameter.vertices,
        large_helix_parameter.vertices
    );
    for radius in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
        let mut mesh = RibbonMesh::default();
        assert!(
            mesh.generate_styled(
                &points,
                &entities,
                &[SecondaryStructure::AlphaHelix; 3],
                RibbonParams {
                    profile: SplineProfile::Rocket,
                    helix_radius: radius,
                    ..RibbonParams::default()
                }
            )
            .is_err()
        );
    }
}
