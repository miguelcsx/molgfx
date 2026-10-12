use super::*;
use crate::cartoon::ribbon::SplineProfile;
use molgfx_core::AtomSelection;
use std::fmt::Write as _;

fn source() -> molframe::Structure {
    let mut cif = String::from(
        "data_sheet\nloop_\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n",
    );
    for row in 0..6 {
        let x = f64::from(row) * 2.0;
        let y = if row % 2 == 0 { -0.5 } else { 0.5 };
        writeln!(cif, "{} C CA GLY A {} {x} {y} 0", row + 1, row + 1).expect("fixture");
    }
    molframe::read_bytes(
        cif.into_bytes(),
        Some("sheet.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("sheet")
    .0
}

fn generate(selection: &AtomSelection) -> RibbonMesh {
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(
        &source(),
        selection,
        &[
            SecondaryStructure::Coil,
            SecondaryStructure::Strand,
            SecondaryStructure::Strand,
            SecondaryStructure::Strand,
            SecondaryStructure::Strand,
            SecondaryStructure::Coil,
        ],
        8.0,
        RibbonParams {
            profile: SplineProfile::Rocket,
            ..RibbonParams::default()
        },
    )
    .expect("selected cartoon");
    mesh.prepare_helix_fits().expect("source-backed frames");
    mesh
}

#[test]
fn a_selected_beta_residue_keeps_complete_strand_frames_and_no_neighbor_owned_geometry() {
    let full = generate(&AtomSelection::All);
    let selected = generate(&AtomSelection::Sparse(vec![2]));
    assert!(
        selected
            .vertices
            .iter()
            .all(|vertex| EntityId(vertex.entity_id).unpack() == Some((EntityKind::Atom, 2)))
    );
    assert!(
        selected
            .deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 15)
    );
    assert_eq!(
        selected.vertices.len(),
        40,
        "eight side quads and two end faces"
    );
    assert_eq!(selected.indices.len(), 60);
    assert_eq!(selected.strand_fits.len(), 1);
    assert_eq!(selected.strand_fits[0].rows, [1, 2, 3, 4]);
    assert_eq!(selected.fit_jobs[2], STRAND_JOB | 4);
    let expected: Vec<_> = full
        .vertices
        .iter()
        .zip(&full.deformations)
        .filter(|(vertex, recipe)| {
            EntityId(vertex.entity_id).unpack() == Some((EntityKind::Atom, 2))
                && recipe.parameter[3].to_bits() == 15
        })
        .map(|(vertex, _)| vertex)
        .collect();
    assert_eq!(expected.len(), 32);
    assert_eq!(
        &selected.vertices[..32],
        expected.into_iter().copied().collect::<Vec<_>>()
    );
    for vertex in &selected.vertices {
        assert!(Vec3::from_array(vertex.position).is_finite());
        assert!((Vec3::from_array(vertex.normal).length() - 1.0).abs() < 1e-5);
    }
}

#[test]
fn selecting_only_a_coil_still_binds_the_hidden_strands_true_join_boundary() {
    let coil = generate(&AtomSelection::Sparse(vec![0]));
    assert!(!coil.vertices.is_empty());
    assert!(
        coil.vertices
            .iter()
            .all(|vertex| EntityId(vertex.entity_id).unpack() == Some((EntityKind::Atom, 0)))
    );
    assert!(
        coil.deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 14)
    );
    assert_eq!(coil.strand_fits[0].rows, [1, 2, 3, 4]);
    assert!(
        coil.deformations
            .iter()
            .flat_map(|recipe| recipe.controls)
            .any(|offset| {
                let properties = &coil.traces.properties;
                properties[offset as usize + 1].to_bits() == 1
                    && properties[offset as usize + 3].to_bits() == 2
                    && properties[offset as usize].to_bits() == coil.strand_fits[0].destination
                    && properties[offset as usize + 2].to_bits() == 60
            })
    );
}

#[test]
fn disjoint_selected_beta_cells_share_one_frame_job_and_close_both_gaps() {
    let mesh = generate(&AtomSelection::Sparse(vec![1, 3]));
    assert_eq!(mesh.strand_fits.len(), 1);
    assert_eq!(mesh.vertices.len(), 80);
    assert_eq!(mesh.indices.len(), 120);
    assert!(mesh.vertices.iter().all(|vertex| matches!(
        EntityId(vertex.entity_id).unpack(),
        Some((EntityKind::Atom, 1 | 3))
    )));
    for row in [1, 3] {
        let faces: HashSet<_> = mesh
            .vertices
            .iter()
            .zip(&mesh.deformations)
            .filter(|(vertex, _)| {
                EntityId(vertex.entity_id).unpack() == Some((EntityKind::Atom, row))
            })
            .map(|(_, recipe)| recipe.controls[3])
            .collect();
        assert_eq!(faces, HashSet::from([0, 1, 2, 3, 4, 5]));
    }
}

#[test]
fn arrow_sides_tilt_their_normals_toward_the_tip_and_keep_the_authored_depth() {
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(
        &source(),
        &AtomSelection::Sparse(vec![4]),
        &[
            SecondaryStructure::Coil,
            SecondaryStructure::Strand,
            SecondaryStructure::Strand,
            SecondaryStructure::Strand,
            SecondaryStructure::Strand,
            SecondaryStructure::Coil,
        ],
        8.0,
        RibbonParams {
            profile: SplineProfile::Rocket,
            thickness: 1.4,
            ..RibbonParams::default()
        },
    )
    .expect("arrow");
    let output = strand_frames::frames(&mesh.traces.points[1..5]);
    let tangent = (output[4].center - output[3].center).normalize();
    for (vertex, recipe) in mesh.vertices.iter().zip(&mesh.deformations) {
        assert_eq!(recipe.parameter[3].to_bits(), 15);
        assert_eq!(recipe.controls[3] & 8, 8);
        let face = recipe.controls[3] & 7;
        if face == 0 || face == 1 {
            assert!(Vec3::from_array(vertex.normal).dot(tangent) > 0.0);
        }
        assert!((recipe.parameter[2].abs() - 0.7).abs() < 1e-6);
    }
}

#[test]
fn a_beta_bridge_inside_a_long_classic_sheet_owns_a_discrete_residue_cell() {
    let entities = [0, 1, 2].map(|row| EntityId::pack(EntityKind::Atom, row).expect("atom").0);
    let points = [Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0), Vec3::X * 2.0];
    let styles = [
        SecondaryStructure::Strand,
        SecondaryStructure::BetaBridge,
        SecondaryStructure::Strand,
    ];
    let mut mesh = RibbonMesh::default();
    mesh.generate_styled(
        &points,
        &entities,
        &styles,
        RibbonParams {
            profile: SplineProfile::Rocket,
            ..RibbonParams::default()
        },
    )
    .expect("classic sheet");
    assert_eq!(mesh.strand_fits[0].rows, [0, 1, 2]);
    assert!(
        mesh.deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 15)
    );
    assert_eq!(
        mesh.vertices
            .iter()
            .filter(|vertex| vertex.entity_id == entities[1])
            .count(),
        36
    );
    let short = [SecondaryStructure::Strand, SecondaryStructure::BetaBridge];
    mesh.generate_styled(
        &points[..2],
        &entities[..2],
        &short,
        RibbonParams {
            profile: SplineProfile::Rocket,
            ..RibbonParams::default()
        },
    )
    .expect("short coil");
    assert!(mesh.strand_fits.is_empty());
    assert!(
        mesh.deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 14)
    );
    for stencil in mesh.traces.properties.as_chunks::<5>().0 {
        assert_eq!(
            stencil[3].to_bits(),
            0,
            "short sheets stay unfiltered on the GPU"
        );
    }
}
