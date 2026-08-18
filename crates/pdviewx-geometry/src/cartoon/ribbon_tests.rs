use super::*;

fn trace() -> [Vec3; 5] {
    [
        Vec3::ZERO,
        Vec3::new(1.0, 0.2, 0.0),
        Vec3::new(2.0, 1.0, 0.2),
        Vec3::new(3.0, 1.2, 1.0),
        Vec3::new(4.0, 0.5, 1.5),
    ]
}

#[test]
fn ribbon_generation_is_deterministic_and_indexed() {
    let mut first = RibbonMesh::default();
    first.generate(&trace(), &[10, 11, 12, 13, 14], RibbonParams::default());
    let mut second = RibbonMesh::default();
    second.generate(&trace(), &[10, 11, 12, 13, 14], RibbonParams::default());
    assert_eq!(first.vertices, second.vertices);
    assert_eq!(first.indices, second.indices);
    assert_eq!(first.vertices.len() % PROFILE_SIDES, 0);
    assert_eq!(
        first.indices.len(),
        (first.vertices.len() / PROFILE_SIDES - 1) * 48
    );
}

#[test]
fn ribbon_vertices_remain_two_sixteen_byte_lanes() {
    assert_eq!(std::mem::size_of::<RibbonVertex>(), 32);
    assert_eq!(std::mem::align_of::<RibbonVertex>(), 4);
}

#[test]
fn ribbon_vertices_are_finite_and_keep_residue_anchors() {
    let mut mesh = RibbonMesh::default();
    mesh.generate(&trace(), &[20, 21, 22, 23, 24], RibbonParams::default());
    assert!(mesh.vertices.iter().all(|vertex| {
        Vec3::from(vertex.position).is_finite()
            && Vec3::from(vertex.normal).is_normalized()
            && (20..=24).contains(&vertex.entity_id)
    }));
    assert!(
        mesh.indices
            .iter()
            .all(|&index| usize::try_from(index).is_ok_and(|i| i < mesh.vertices.len()))
    );
}

#[test]
fn fewer_than_two_trace_points_produce_no_geometry() {
    let mut mesh = RibbonMesh::default();
    mesh.generate(&[Vec3::ZERO], &[3], RibbonParams::default());
    assert!(mesh.vertices.is_empty());
    assert!(mesh.indices.is_empty());
}

#[test]
fn tube_profile_has_a_constant_round_cross_section() {
    let mut mesh = RibbonMesh::default();
    mesh.generate(
        &trace(),
        &[20, 21, 22, 23, 24],
        RibbonParams {
            width: 0.8,
            thickness: 0.8,
            profile: SplineProfile::Tube,
            ..RibbonParams::default()
        },
    );
    let diameters = profile_diameters(&mesh.vertices);
    assert!((diameters.0 - 0.8).abs() < 1.0e-5);
    assert!((diameters.1 - 0.8).abs() < 1.0e-5);
}

#[test]
fn putty_profile_maps_recorded_b_factors_to_interpolated_round_radii() {
    let structure = polymer_structure();
    let mapping = match TubeRadiusMapping::b_factor([10.0, 50.0], [0.2, 0.8]) {
        Ok(mapping) => mapping,
        Err(error) => panic!("putty mapping builds: {error}"),
    };
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(
        &structure,
        &pdviewx_core::AtomSelection::All,
        &[SecondaryStructure::Coil; 3],
        8.0,
        RibbonParams {
            width: 0.6,
            thickness: 0.6,
            profile: SplineProfile::Tube,
            radius_mapping: mapping,
            ..RibbonParams::default()
        },
    );
    let rings = mesh
        .vertices
        .chunks_exact(PROFILE_SIDES)
        .collect::<Vec<_>>();
    let first = profile_diameters(rings.first().copied().unwrap_or(&[]));
    let last = profile_diameters(rings.last().copied().unwrap_or(&[]));
    assert!((first.0 - 0.4).abs() < 1.0e-4);
    assert!((last.0 - 1.6).abs() < 1.0e-4);
    assert!(
        rings
            .windows(2)
            .all(|pair| { profile_diameters(pair[0]).0 <= profile_diameters(pair[1]).0 + 1.0e-5 })
    );
}

#[test]
fn secondary_structure_changes_cross_section_without_changing_topology() {
    let trace = trace();
    let entities = [10, 11, 12, 13, 14];
    let mut helix = RibbonMesh::default();
    helix.generate_styled(
        &trace,
        &entities,
        &[SecondaryStructure::Helix; 5],
        RibbonParams::default(),
    );
    let mut strand = RibbonMesh::default();
    strand.generate_styled(
        &trace,
        &entities,
        &[SecondaryStructure::Strand; 5],
        RibbonParams::default(),
    );

    assert_eq!(helix.indices, strand.indices);
    assert_eq!(helix.vertices.len(), strand.vertices.len());
    let helix_profile = profile_diameters(&helix.vertices);
    let strand_profile = profile_diameters(&strand.vertices);
    assert!(strand_profile.0 > helix_profile.0);
    assert!(strand_profile.1 < helix_profile.1);
}

#[test]
fn trajectory_samples_move_ribbons_without_materializing_a_coordinate_frame() {
    let structure = polymer_structure();
    let start = [[0.0, 0.0, 0.0], [2.0, 0.4, 0.0], [4.0, 0.0, 0.0]];
    let end = [[0.0, 4.0, 0.0], [2.0, 4.4, 0.0], [4.0, 4.0, 0.0]];
    let mut initial = RibbonMesh::default();
    initial.generate_structure_interpolated(
        &structure,
        &pdviewx_core::AtomSelection::All,
        &[SecondaryStructure::Coil; 3],
        8.0,
        InterpolatedCoordinates {
            start: &start,
            end: &end,
            alpha: 0.0,
        },
        RibbonParams::default(),
    );
    let mut midpoint = RibbonMesh::default();
    midpoint.generate_structure_interpolated(
        &structure,
        &pdviewx_core::AtomSelection::All,
        &[SecondaryStructure::Coil; 3],
        8.0,
        InterpolatedCoordinates {
            start: &start,
            end: &end,
            alpha: 0.5,
        },
        RibbonParams::default(),
    );
    assert_eq!(initial.indices, midpoint.indices);
    assert_eq!(initial.vertices.len(), midpoint.vertices.len());
    assert!(
        initial
            .vertices
            .iter()
            .zip(&midpoint.vertices)
            .all(|(a, b)| {
                (b.position[0] - a.position[0]).abs() < 1.0e-5
                    && (b.position[1] - a.position[1] - 2.0).abs() < 1.0e-5
                    && (b.position[2] - a.position[2]).abs() < 1.0e-5
            })
    );
}

fn polymer_structure() -> pdbiox::Structure {
    let cif = "\
data_polymer
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
ATOM 1 C CA . GLY A 1 1 0.0 0.0 0.0 1.00 10.0 1 A 1
ATOM 2 C CA . ALA A 1 2 2.0 0.4 0.0 1.00 30.0 2 A 1
ATOM 3 C CA . SER A 1 3 4.0 0.0 0.0 1.00 50.0 3 A 1
";
    match pdbiox::read_bytes(
        cif.as_bytes().to_vec(),
        Some("polymer.cif"),
        &pdbiox::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("polymer fixture parses: {diagnostics:?}"),
    }
}

fn profile_diameters(vertices: &[RibbonVertex]) -> (f32, f32) {
    let across_width = Vec3::from(vertices[0].position).distance(Vec3::from(vertices[4].position));
    let across_thickness =
        Vec3::from(vertices[2].position).distance(Vec3::from(vertices[6].position));
    (
        across_width.max(across_thickness),
        across_width.min(across_thickness),
    )
}
