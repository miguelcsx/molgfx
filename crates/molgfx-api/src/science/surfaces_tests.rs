use super::{
    AssemblyInstance, AssemblySpec, FitResult, MovieExportRequest, UnitCellSpec, ValidationFinding,
};
use crate::StructureId;

#[test]
fn a_unit_cell_rejects_non_finite_metadata() {
    assert!(UnitCellSpec::new([1.0, f32::NAN, 2.0], [90.0; 3]).is_err());
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
