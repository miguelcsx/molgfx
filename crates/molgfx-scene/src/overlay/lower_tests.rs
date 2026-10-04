//! Lowering bound overlay data to renderer handles.

use crate::appearance::tests::two_chains;
use crate::overlay::UnitCellSpec;
use crate::{AssemblySpec, DataSource, PatchOperation, Scene, ScenePatch, VolumeBinding, density};
use std::sync::Arc;

#[test]
fn bound_volume_lowers_to_a_real_renderer_handle() {
    let mut scene = Scene::from_structure(&two_chains()).unwrap_or_else(|error| panic!("{error}"));
    let source = DataSource::new("density");
    let _ = scene
        .add(density::volume(source.clone(), [2, 2, 2]).isosurface(
            2.5,
            crate::Color::rgb(49, 104, 142),
            1.0,
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    let values: Arc<[f32]> = (0_u8..8).map(f32::from).collect();
    scene
        .bind_volume(VolumeBinding::new(source, [2, 2, 2], values))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.overlay_handles().volumes, 1);
}

/// One alanine whose CA carries an anisotropic displacement tensor; the rest
/// of the backbone is isotropic.
const ANISO_PDB: &str = "\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N\n\
ATOM      2  CA  ALA A   1       1.458   0.000   0.000  1.00  0.00           C\n\
ANISOU    2  CA  ALA A   1   100000 200000 300000  40000  50000  60000       C\n\
ATOM      3  C   ALA A   1       2.009   1.420   0.000  1.00  0.00           C\n\
END\n";

fn aniso_scene() -> Scene {
    let structure = match molframe::read_bytes(
        ANISO_PDB.as_bytes().to_vec(),
        Some("aniso.pdb"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(error) => panic!("anisotropy fixture must parse: {error:?}"),
    };
    Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn an_adp_overlay_emits_one_ellipsoid_per_tensor_bearing_atom() {
    let mut scene = aniso_scene();
    let structure = crate::StructureId::new(1);
    let _ = scene
        .add(crate::ellipsoid::adp(
            structure,
            crate::Selection::from("all"),
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    // Only the CA carries a tensor, so exactly one ellipsoid is emitted even
    // though every atom is selected.
    assert_eq!(scene.overlay_handles().ellipsoids, 1);
}

#[test]
fn an_isotropic_structure_emits_no_ellipsoids() {
    let mut scene = Scene::from_structure(&two_chains()).unwrap_or_else(|error| panic!("{error}"));
    let structure = crate::StructureId::new(1);
    let _ = scene
        .add(crate::ellipsoid::adp(
            structure,
            crate::Selection::from("all"),
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.overlay_handles().ellipsoids, 0);
}

#[test]
fn an_ellipsoid_overlay_rejects_a_nonpositive_scale() {
    let mut scene = aniso_scene();
    let structure = crate::StructureId::new(1);
    let mut spec = crate::ellipsoid::adp(structure, crate::Selection::from("all"));
    spec.scale = 0.0;
    assert!(scene.add(spec).is_err());
}

#[test]
fn assembly_unit_cell_rebinds_to_exactly_twelve_guides() {
    let mut scene = Scene::from_structure(&two_chains()).unwrap_or_else(|error| panic!("{error}"));
    let structure = crate::StructureId::new(1);
    let unit_cell =
        UnitCellSpec::new([10.0, 11.0, 12.0], [90.0; 3]).unwrap_or_else(|error| panic!("{error}"));
    let assembly = AssemblySpec {
        structures: vec![structure],
        instances: Vec::new(),
        unit_cell: Some(unit_cell),
    };
    let patch = ScenePatch {
        base_revision: scene.spec().revision,
        operations: vec![PatchOperation::SetAssembly {
            assembly: Some(assembly),
        }],
    };
    scene
        .apply(&patch)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.overlay_handles().unit_cell_guides, 12);

    let clear = ScenePatch {
        base_revision: scene.spec().revision,
        operations: vec![PatchOperation::SetAssembly { assembly: None }],
    };
    scene
        .apply(&clear)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.overlay_handles().unit_cell_guides, 0);
}
#[test]
fn plane_add_and_remove_rebinds_exactly_four_guides() {
    let mut scene = Scene::from_structure(&two_chains()).unwrap_or_else(|error| panic!("{error}"));
    let id = scene
        .add(crate::PlaneSpec::new(
            crate::StructureId::new(1),
            [0.0; 3],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [4.0, 6.0],
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.overlay_handles().plane_guides, 4);
    assert!(scene.spec().planes.contains_key(&id));
    let patch = ScenePatch {
        base_revision: scene.spec().revision,
        operations: vec![PatchOperation::RemovePlane { id }],
    };
    scene
        .apply(&patch)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.overlay_handles().plane_guides, 0);
    assert!(!scene.spec().planes.contains_key(&id));
}
#[test]
fn invalid_plane_does_not_change_the_scene_or_its_guides() {
    let mut scene = Scene::from_structure(&two_chains()).unwrap_or_else(|error| panic!("{error}"));
    let base = scene.to_spec();
    let mut plane = crate::PlaneSpec::new(
        crate::StructureId::new(1),
        [0.0; 3],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [4.0, 6.0],
    );
    plane.tangent = plane.normal;
    assert!(matches!(
        scene.add(plane),
        Err(crate::Error::InvalidSpec(_))
    ));
    assert_eq!(scene.spec(), &base);
    assert_eq!(scene.overlay_handles().plane_guides, 0);

    plane.tangent = [1.0, 0.0, 0.0];
    plane.structure = crate::StructureId::new(100);
    assert!(matches!(
        scene.add(plane),
        Err(crate::Error::InvalidSpec(_))
    ));
    assert_eq!(scene.spec(), &base);
    assert_eq!(scene.overlay_handles().plane_guides, 0);
}

#[test]
fn removing_a_plane_and_cell_can_be_undone_without_losing_geometry() {
    let mut scene = Scene::from_structure(&two_chains()).unwrap_or_else(|error| panic!("{error}"));
    let id = scene
        .add(crate::PlaneSpec::new(
            crate::StructureId::new(1),
            [1.0, 2.0, 3.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 0.0],
            [4.0, 6.0],
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    let cell = UnitCellSpec::new([10.0, 11.0, 12.0], [80.0, 90.0, 100.0])
        .unwrap_or_else(|error| panic!("{error}"));
    let assembly = AssemblySpec {
        structures: vec![crate::StructureId::new(1)],
        instances: Vec::new(),
        unit_cell: Some(cell),
    };
    scene
        .apply(&ScenePatch {
            base_revision: scene.spec().revision,
            operations: vec![PatchOperation::SetAssembly {
                assembly: Some(assembly),
            }],
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let before = scene.to_spec();
    assert_eq!(scene.overlay_handles().plane_guides, 4);
    assert_eq!(scene.overlay_handles().unit_cell_guides, 12);

    let patch = ScenePatch {
        base_revision: before.revision,
        operations: vec![
            PatchOperation::RemovePlane { id },
            PatchOperation::SetAssembly { assembly: None },
        ],
    };
    let inverse = patch
        .inverse(&before)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .apply(&patch)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.overlay_handles().plane_guides, 0);
    assert_eq!(scene.overlay_handles().unit_cell_guides, 0);
    scene
        .apply(&inverse)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.spec().planes, before.planes);
    assert_eq!(scene.spec().assembly, before.assembly);
    assert_eq!(scene.overlay_handles().plane_guides, 4);
    assert_eq!(scene.overlay_handles().unit_cell_guides, 12);
}
