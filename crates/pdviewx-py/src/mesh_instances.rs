//! Shared-mesh instance authoring and lifecycle controls.

use crate::core::{PyMeshHandle, PyMeshInstanceHandle, PyScene};
use crate::error::{core, value};
use crate::math::PyMat4;
use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;

#[pymethods]
impl PyScene {
    fn add_mesh_instance(
        &mut self,
        mesh: PyMeshHandle,
        transform: PyMat4,
    ) -> PyResult<PyMeshInstanceHandle> {
        let instance = core(pdviewx::MeshInstance::new(mesh.0, transform.0))?;
        core(self.inner.add_mesh_instance(instance)).map(Into::into)
    }

    fn copy_mesh_instances_from_numpy(
        &mut self,
        mesh: PyMeshHandle,
        transforms: PyReadonlyArray2<'_, f32>,
    ) -> PyResult<Option<PyMeshInstanceHandle>> {
        if transforms.shape().get(1).copied() != Some(16) {
            return Err(value("transforms must have shape (N, 16)"));
        }
        let transforms = transforms
            .as_slice()
            .map_err(|_| value("transforms must be C-contiguous float32"))?;
        let mut last = None;
        for lanes in transforms.chunks_exact(16) {
            let matrix = pdviewx::Mat4::from_cols_array(&[
                lanes[0], lanes[1], lanes[2], lanes[3], lanes[4], lanes[5], lanes[6], lanes[7],
                lanes[8], lanes[9], lanes[10], lanes[11], lanes[12], lanes[13], lanes[14],
                lanes[15],
            ]);
            let instance = core(pdviewx::MeshInstance::new(mesh.0, matrix))?;
            last = Some(core(self.inner.add_mesh_instance(instance))?.into());
        }
        Ok(last)
    }

    fn set_mesh_instance_transform(
        &mut self,
        handle: PyMeshInstanceHandle,
        transform: PyMat4,
    ) -> PyResult<bool> {
        let Some(instance) = self.inner.mesh_instance_mut(handle.0) else {
            return Ok(false);
        };
        core(instance.set_transform(transform.0))?;
        Ok(true)
    }

    fn set_mesh_instance_visible(&mut self, handle: PyMeshInstanceHandle, visible: bool) -> bool {
        let Some(instance) = self.inner.mesh_instance_mut(handle.0) else {
            return false;
        };
        instance.set_visible(visible);
        true
    }

    fn remove_mesh_instance(&mut self, handle: PyMeshInstanceHandle) -> bool {
        self.inner.remove_mesh_instance(handle.0).is_some()
    }
}
