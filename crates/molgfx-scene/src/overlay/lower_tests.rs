//! Lowering bound overlay data to renderer handles.

use crate::appearance::tests::two_chains;
use crate::{DataSource, Scene, VolumeBinding, density};
use std::sync::Arc;

#[test]
fn bound_volume_lowers_to_a_real_renderer_handle() {
    let mut scene = Scene::from_structure(&two_chains()).unwrap_or_else(|error| panic!("{error}"));
    let source = DataSource::new("density");
    let _ = scene
        .add(density::volume(source.clone(), [2, 2, 2]).isovalue(2.5))
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
