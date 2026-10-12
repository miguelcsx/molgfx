//! Runtime binding edits publish after releasing the authoritative scene borrow.
use crate::{
    binding::error,
    scene_binding::{PyScene, deliver},
};
use pyo3::{exceptions::PyValueError, prelude::*};
use std::sync::Arc;

pub(super) fn commit(
    slf: &Bound<'_, PyScene>,
    py: Python<'_>,
    mutation: impl FnOnce(&mut molgfx::Scene) -> PyResult<()>,
) -> PyResult<()> {
    let patch = {
        let mut scene = slf.borrow_mut();
        if scene.pending.is_some() {
            return Err(PyValueError::new_err(
                "runtime bindings cannot be changed inside a scene transaction",
            ));
        }
        let base_revision = scene.inner.revision();
        mutation(&mut scene.inner)?;
        molgfx::ScenePatch {
            base_revision,
            operations: Vec::new(),
        }
    };
    deliver(slf, py, &patch)
}

#[pymethods]
impl PyScene {
    /// Binds classified donor, hydrogen and acceptor atom rows to an interaction.
    #[pyo3(signature = (identity, endpoints, *, candidate_distance=None, pose=None))]
    fn bind_hydrogen_bonds(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        identity: &Bound<'_, PyAny>,
        endpoints: Vec<[u32; 3]>,
        candidate_distance: Option<f32>,
        pose: Option<&crate::molecular_surface_binding::PyMolecularPose>,
    ) -> PyResult<()> {
        let id = crate::id_binding::interaction_id(identity)?;
        commit(slf, py, |scene| {
            let Some(molgfx::InteractionSpec::HydrogenBonds(spec)) = scene
                .spec()
                .interactions
                .get(&molgfx::InteractionId::new(id))
            else {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "identity is not a hydrogen-bond table presentation",
                ));
            };
            let binding = match candidate_distance {
                Some(distance) => molgfx::HydrogenBondBinding::candidates(
                    spec.source.clone(),
                    Arc::from(endpoints),
                    distance,
                ),
                None => molgfx::HydrogenBondBinding::new(spec.source.clone(), Arc::from(endpoints)),
            };
            let binding = match pose {
                Some(pose) => binding.at_pose(pose.0.clone()),
                None => binding,
            };
            scene.bind_hydrogen_bonds(binding).map_err(error)
        })
    }

    fn bind_volume(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        identity: &Bound<'_, PyAny>,
        values: Vec<f32>,
    ) -> PyResult<()> {
        let identity = identity.extract::<PyRef<'_, crate::id_binding::PyVolumeId>>()?;
        commit(slf, py, |scene| {
            let spec = scene
                .spec()
                .volumes
                .get(&molgfx::VolumeId::new(identity.0))
                .ok_or_else(|| PyValueError::new_err("unknown volume identity"))?;
            scene
                .bind_volume(
                    molgfx::VolumeBinding::new(
                        spec.source.clone(),
                        spec.dimensions,
                        Arc::from(values),
                    )
                    .affine(spec.voxel_to_world),
                )
                .map_err(error)
        })
    }
}
