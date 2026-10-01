use super::{
    AssemblyInstance, AssemblySpec, FitResult, MovieExportRequest, UnitCellSpec, ValidationFinding,
};
use crate::StructureId;

#[test]
fn a_unit_cell_rejects_non_finite_metadata() {
    assert!(UnitCellSpec::new([1.0, f32::NAN, 2.0], [90.0; 3]).is_err());
}

#[test]
fn unit_cell_construction_rejects_impossible_angles_and_core_degeneracy() {
    for (lengths, angles) in [([1.0; 3], [10.0, 10.0, 170.0]), ([1.0e-8; 3], [90.0; 3])] {
        assert!(matches!(
            UnitCellSpec::new(lengths, angles),
            Err(crate::Error::InvalidSpec(_))
        ));
    }
}

#[test]
fn deserialized_assemblies_reject_geometrically_impossible_unit_cells() {
    let assembly: AssemblySpec = serde_json::from_value(serde_json::json!({
        "structures": [1], "instances": [],
        "unit_cell": {"lengths": [1.0, 1.0, 1.0], "angles_degrees": [10.0, 10.0, 170.0], "origin": [0.0, 0.0, 0.0]}
    })).unwrap();
    assert!(matches!(
        assembly.validate(),
        Err(crate::Error::InvalidSpec(_))
    ));
}

#[test]
fn portable_unit_cell_validation_matches_construction_for_skew_and_invalid_boundaries() {
    for (lengths, angles) in [
        ([10.0, 11.0, 12.0], [80.0, 90.0, 100.0]),
        ([1.0; 3], [60.0, 60.0, 120.0]),
        ([1.0; 3], [10.0, 10.0, 170.0]),
        ([1.0e-8; 3], [90.0; 3]),
    ] {
        let restored: UnitCellSpec = serde_json::from_value(serde_json::json!({
            "lengths": lengths, "angles_degrees": angles, "origin": [0.0, 0.0, 0.0]
        }))
        .unwrap();
        assert_eq!(
            restored.validate().is_ok(),
            UnitCellSpec::new(lengths, angles).is_ok()
        );
    }
    let valid = UnitCellSpec::new([10.0, 11.0, 12.0], [80.0, 90.0, 100.0]).unwrap();
    for origin in [[f32::NAN; 3], [f32::INFINITY; 3], [f32::MAX; 3]] {
        assert!(matches!(
            valid.with_origin(origin),
            Err(crate::Error::InvalidSpec(_))
        ));
        let direct = UnitCellSpec { origin, ..valid };
        assert!(matches!(
            direct.validate(),
            Err(crate::Error::InvalidSpec(_))
        ));
    }
}

#[test]
fn an_invalid_cell_patch_preserves_existing_plane_geometry_and_semantic_state() {
    use crate::{PatchOperation, PlaneSpec, Scene, ScenePatch};
    let mut scene = Scene::from_structure(&crate::appearance::tests::two_chains()).unwrap();
    scene
        .add(PlaneSpec::new(
            StructureId::new(1),
            [0.0; 3],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [4.0, 2.0],
        ))
        .unwrap();
    let before = scene.to_spec();
    let guides: Vec<_> = scene
        .resolved()
        .guides()
        .map(|(handle, guide)| (handle, *guide))
        .collect();
    let revision = scene.resolved().guide_revision();
    let assembly = AssemblySpec {
        structures: vec![StructureId::new(1)],
        instances: Vec::new(),
        unit_cell: Some(UnitCellSpec {
            lengths: [1.0; 3],
            angles_degrees: [10.0, 10.0, 170.0],
            origin: [0.0; 3],
        }),
    };
    let patch = ScenePatch {
        base_revision: scene.revision(),
        operations: vec![PatchOperation::SetAssembly {
            assembly: Some(assembly),
        }],
    };
    assert!(matches!(
        scene.apply(&patch),
        Err(crate::Error::InvalidSpec(_))
    ));
    assert_eq!(scene.spec(), &before);
    assert_eq!(scene.resolved().guide_revision(), revision);
    assert_eq!(
        scene
            .resolved()
            .guides()
            .map(|(handle, guide)| (handle, *guide))
            .collect::<Vec<_>>(),
        guides
    );
}

#[test]
fn an_assembly_rejects_duplicate_instance_identity() {
    let owner = StructureId::new(1);
    let instance = AssemblyInstance::new(
        4,
        owner,
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let assembly = AssemblySpec {
        structures: vec![owner],
        instances: vec![instance.clone(), instance],
        unit_cell: None,
    };
    assert!(assembly.validate().is_err());
}

#[test]
fn fitting_and_validation_values_require_provenance() {
    let fit = FitResult {
        source: StructureId::new(1),
        target: StructureId::new(2),
        correspondences: 3,
        rmsd: 0.4,
        transform: [1.0; 16],
        provenance: "kabsch:coordinates-7".into(),
    };
    assert!(fit.validate().is_ok());
    let finding = ValidationFinding {
        structure: StructureId::new(1),
        entity: 8,
        kind: "clash".into(),
        severity: 0.5,
        provenance: "molframe:validation-1".into(),
    };
    assert!(finding.validate().is_ok());
}

#[test]
fn export_requests_are_bounded_and_deterministic() {
    let request = MovieExportRequest {
        first_frame: 2,
        last_frame: 5,
        dimensions: [1280, 720],
        frames_per_second: 24,
        camera_seed: 9,
    };
    assert!(request.validate().is_ok());
}
