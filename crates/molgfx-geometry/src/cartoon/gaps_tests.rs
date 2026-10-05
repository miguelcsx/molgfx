use super::*;
use crate::{RibbonMesh, RibbonParams};

fn source(rows: &[(u32, u32, f32)]) -> molframe::Structure {
    let mut cif = String::from(
        "data_gap\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n\
_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n\
_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n\
_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n\
_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n_atom_site.auth_asym_id\n\
_atom_site.pdbx_PDB_model_num\n",
    );
    for (index, &(chain, sequence, x)) in rows.iter().enumerate() {
        use std::fmt::Write as _;
        writeln!(
            cif,
            "ATOM {} C CA . GLY C{chain} {chain} {sequence} {x} 0 0 1 10 {sequence} C{chain} 1",
            index + 1
        )
        .expect("fixture formats");
    }
    molframe::read_bytes(
        cif.into_bytes(),
        Some("gap.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("fixture parses")
    .0
}

fn mesh(source: &molframe::Structure, selection: &AtomSelection) -> RibbonMesh {
    let mut mesh = RibbonMesh::default();
    mesh.append_polymer_gaps(source, selection, None, RibbonParams::default())
        .expect("gaps generate");
    mesh
}

#[test]
fn canonical_missing_residues_draw_closed_dashes_at_physical_spacing() {
    let source = source(&[(1, 1, 0.0), (1, 5, 4.0)]);
    let mesh = mesh(&source, &AtomSelection::All);
    assert!(!mesh.vertices.is_empty());
    assert!(
        mesh.indices
            .iter()
            .all(|&index| (index as usize) < mesh.vertices.len())
    );
    for vertex in &mesh.vertices {
        let [x, y, z] = vertex.position;
        assert!((y * y + z * z).sqrt() <= GAP_RADIUS + 1e-5);
        assert!(x.rem_euclid(DASH_LENGTH + DASH_SPACING) <= DASH_LENGTH + 1e-5);
    }
    let recipes: &[super::super::ribbon::RibbonDeformation] =
        bytemuck::cast_slice(mesh.deformation_bytes());
    assert!(recipes.iter().all(|recipe| {
        recipe.controls == [0, 1, 0, 1] && recipe.parameter[3].to_bits() == GAP_ANCHOR
    }));
}

#[test]
fn spatial_gaps_draw_without_fabricating_chain_or_selection_connections() {
    assert!(
        !mesh(&source(&[(1, 1, 0.0), (1, 2, 12.0)]), &AtomSelection::All)
            .vertices
            .is_empty()
    );
    assert!(
        mesh(&source(&[(1, 1, 0.0), (2, 2, 12.0)]), &AtomSelection::All)
            .vertices
            .is_empty()
    );
    let source = source(&[(1, 1, 0.0), (1, 2, 4.0), (1, 3, 12.0)]);
    let selection = AtomSelection::Sparse(vec![0, 2]);
    assert!(mesh(&source, &selection).vertices.is_empty());
}

#[test]
fn continuous_backbones_have_no_gap_geometry() {
    assert!(
        mesh(&source(&[(1, 1, 0.0), (1, 2, 4.0)]), &AtomSelection::All)
            .vertices
            .is_empty()
    );
}

#[test]
fn resident_trajectory_endpoints_reserve_dashes_for_the_entire_interval() {
    use molgfx_core::TrajectoryFrame;
    use std::sync::Arc;
    let source = source(&[(1, 1, 0.0), (1, 5, 4.0)]);
    let start = TrajectoryFrame::new(0, 0.0, Arc::from([[0.0; 3], [4.0, 0.0, 0.0]]), "start")
        .expect("start validates");
    let end = TrajectoryFrame::new(1, 1.0, Arc::from([[0.0; 3], [12.0, 0.0, 0.0]]), "end")
        .expect("end validates");
    let trajectory = TrajectorySegment::new(start, end, 0.5).expect("interval validates");
    let mut mesh = RibbonMesh::default();
    mesh.append_polymer_gaps(
        &source,
        &AtomSelection::All,
        Some(&trajectory),
        RibbonParams::default(),
    )
    .expect("gap capacity covers the interval");
    let recipes: &[super::super::ribbon::RibbonDeformation] =
        bytemuck::cast_slice(mesh.deformation_bytes());
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe.parameter[1])
            .fold(0.0, f32::max)
            .to_bits(),
        11.0_f32.to_bits()
    );
}

#[test]
fn oversized_gaps_fail_before_emitting_partial_geometry() {
    let source = source(&[(1, 1, 0.0), (1, 5, 100_000_000.0)]);
    let mut mesh = RibbonMesh::default();
    assert!(matches!(
        mesh.append_polymer_gaps(&source, &AtomSelection::All, None, RibbonParams::default()),
        Err(CartoonError::Packing(
            crate::PackingError::IndexOverflow { .. }
        ))
    ));
    assert!(mesh.vertices.is_empty());
    assert!(mesh.indices.is_empty());
    assert!(mesh.deformation_bytes().is_empty());
}
