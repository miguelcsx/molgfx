use super::*;
use pdviewx_core::TubeRadiusMapping;

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
fn putty_lowering_keeps_base_geometry_constant_and_emits_compact_guide_values() {
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
            ..RibbonParams::default()
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(mesh.radius_source_values(), &[10.0, 30.0, 50.0]);
    assert!(
        mesh.vertices
            .chunks_exact(PROFILE_SIDES)
            .all(|ring| (profile_diameters(ring).0 - 0.6).abs() < 1.0e-4),
        "putty radius is deferred to the vertex shader"
    );
    let first = mesh.deformations[0];
    let controls = [first.parameter[1].to_bits(), first.parameter[2].to_bits()];
    assert_eq!(controls, [0, 1]);
    assert!(
        (crate::cartoon::variable_tube_radius(
            mapping,
            mesh.radius_source_values(),
            controls,
            0.5,
            0.3,
        ) - 0.35)
            .abs()
            < 1.0e-6
    );
}

#[test]
fn putty_cpu_reference_matches_missing_value_shader_policy() {
    let mapping = match TubeRadiusMapping::b_factor([0.0, 10.0], [0.2, 1.2]) {
        Ok(mapping) => mapping,
        Err(error) => panic!("putty mapping builds: {error}"),
    };
    let values = [f32::NAN, 5.0, f32::NAN];
    assert!(
        (crate::cartoon::variable_tube_radius(mapping, &values, [0, 1], 0.25, 0.4) - 0.7).abs()
            < 1.0e-6
    );
    assert!(
        (crate::cartoon::variable_tube_radius(mapping, &values, [0, 2], 0.5, 0.4) - 0.4).abs()
            < f32::EPSILON
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
fn structure_ribbons_emit_one_compact_gpu_recipe_per_vertex() {
    let structure = polymer_structure();
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(
        &structure,
        &pdviewx_core::AtomSelection::All,
        &[SecondaryStructure::Coil; 3],
        8.0,
        RibbonParams::default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert!(!mesh.vertices.is_empty());
    assert_eq!(mesh.deformation_bytes().len(), mesh.vertices.len() * 32);
    let controls: &[u32] = bytemuck::cast_slice(mesh.deformation_bytes());
    assert_ne!(controls[0], u32::MAX, "polymer rows are GPU-addressable");
}

#[test]
fn twister_faces_use_distinct_orientation_colours() {
    let base = Rgba8::new(1, 2, 3, 200);
    let top = profile_color(SplineProfile::Twister, 1.0, base);
    let bottom = profile_color(SplineProfile::Twister, -1.0, base);
    assert_ne!(top, bottom);
    assert_eq!(top.a, 200);
    assert_eq!(bottom.a, 200);
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

#[test]
fn the_twister_profile_gives_each_face_its_own_flat_normal() {
    fn assert_axis(actual: [f32; 3], expected: [f32; 3], what: &str) {
        assert!(
            (Vec3::from(actual) - Vec3::from(expected)).length() < 1.0e-5,
            "{what}: expected {expected:?}, got {actual:?}"
        );
    }

    let mut mesh = RibbonMesh::default();
    mesh.generate(
        &trace(),
        &[10, 11, 12, 13, 14],
        RibbonParams {
            width: 1.5,
            thickness: 0.24,
            profile: SplineProfile::Twister,
            ..RibbonParams::default()
        },
    );
    let ring = mesh
        .vertices
        .chunks_exact(PROFILE_SIDES)
        .next()
        .unwrap_or(&[]);
    assert_eq!(ring.len(), PROFILE_SIDES);
    // Doubled corners: same position, different normal, which is what makes
    // the edge read as an edge instead of a gradient.
    assert_axis(
        ring[1].position,
        ring[2].position,
        "doubled corner position",
    );
    assert!(
        (Vec3::from(ring[1].normal) - Vec3::from(ring[2].normal)).length() > 0.5,
        "the two faces meeting at a corner carry different normals"
    );
    // A face is flat: both of its corners agree.
    assert_axis(ring[0].normal, ring[1].normal, "face normal");
    assert!(
        ring.iter()
            .all(|vertex| Vec3::from(vertex.normal).is_normalized())
    );
}
