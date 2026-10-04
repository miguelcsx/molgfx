use super::apply;
use crate::{Color, DataSource, PatchOperation, SceneSpec, VolumeSpec};

#[test]
fn set_isovalue_rejects_nonfinite_before_mutating() {
    let id = crate::VolumeId::new(1);
    let mut spec = SceneSpec::empty();
    spec.volumes.insert(
        id,
        VolumeSpec {
            source: DataSource::new("density"),
            dimensions: [2, 2, 2],
            voxel_to_world: molgfx_math::Mat4::IDENTITY.to_cols_array(),
            presentations: vec![crate::VolumePresentation::Isosurface {
                isovalue: 1.0,
                color: Color::rgb(1, 2, 3),
                opacity: 1.0,
                style: crate::IsoStyle::Solid,
            }],
            region: None,
        },
    );
    let before = spec.clone();
    assert!(
        apply(
            &mut spec,
            &PatchOperation::SetVolumeIsovalue {
                id,
                isovalue: f32::NAN
            }
        )
        .is_err()
    );
    assert_eq!(spec, before);
}

#[test]
fn non_overlay_operations_are_left_for_other_patch_domains() {
    let mut spec = SceneSpec::empty();
    let before = spec.clone();
    assert!(matches!(
        apply(&mut spec, &PatchOperation::SetFocus { selection: None }),
        Ok(false)
    ));
    assert_eq!(spec, before);
}

#[test]
fn volume_isovalue_validation_precedes_missing_id_lookup() {
    let mut spec = SceneSpec::empty();
    let before = spec.clone();
    let id = crate::VolumeId::new(1);
    assert!(matches!(
        apply(
            &mut spec,
            &PatchOperation::SetVolumeIsovalue {
                id,
                isovalue: f32::NAN,
            }
        ),
        Err(crate::Error::InvalidSpec(_))
    ));
    assert!(matches!(
        apply(
            &mut spec,
            &PatchOperation::SetVolumeIsovalue { id, isovalue: 1.0 }
        ),
        Err(crate::Error::Patch(crate::PatchError::MissingId))
    ));
    assert_eq!(spec, before);
}
