//! Inverse operations that undo a patch.

use super::inverse::inverse_operations;
use crate::{Color, DataSource, PatchOperation, SceneSpec, VolumeSpec};

#[test]
fn set_isovalue_inverse_restores_the_base_value() {
    let id = crate::VolumeId::new(1);
    let mut base = SceneSpec::empty();
    base.volumes.insert(
        id,
        VolumeSpec {
            source: DataSource::new("density"),
            dimensions: [2, 2, 2],
            spacing: [1.0; 3],
            origin: [0.0; 3],
            isovalue: 1.25,
            color: Color::rgb(1, 2, 3),
        },
    );
    let inverse = inverse_operations(
        &PatchOperation::SetVolumeIsovalue { id, isovalue: 2.5 },
        &base,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        inverse,
        vec![PatchOperation::SetVolumeIsovalue { id, isovalue: 1.25 }]
    );
}
