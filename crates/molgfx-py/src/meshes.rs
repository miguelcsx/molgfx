//! Caller mesh read-back: the triangles and the owner that places them.

use crate::core::{PyMaterial, PyMeshHandle, PyScene, PyStructureHandle};
use crate::math::{PyAabb, PyRgba8, PyVec3};
use crate::pending_surface::presentation::{PyFaceVisibility, PySurfaceComponentPolicy};
use crate::values::volume::PyClipSet;
use numpy::{PyArray1, PyArray2, PyArrayMethods};
use pyo3::prelude::*;

/// One caller-supplied vertex: model-space position, normal and colour.
#[pyclass(name = "MeshVertex", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyMeshVertex(pub(crate) molgfx::core::MeshVertex);

impl From<molgfx::core::MeshVertex> for PyMeshVertex {
    fn from(value: molgfx::core::MeshVertex) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyMeshVertex {
    #[getter]
    fn position(&self) -> PyVec3 {
        PyVec3(self.0.position)
    }

    #[getter]
    fn normal(&self) -> PyVec3 {
        PyVec3(self.0.normal)
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        PyRgba8(self.0.color)
    }
}

/// A validated triangle mesh owned by one structure.
#[pyclass(name = "Mesh", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyMesh(pub(crate) molgfx::core::Mesh);

impl From<&molgfx::core::Mesh> for PyMesh {
    fn from(value: &molgfx::core::Mesh) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyMesh {
    /// Structure whose placement moves this mesh.
    #[getter]
    fn owner(&self) -> PyStructureHandle {
        self.0.owner().into()
    }

    #[getter]
    fn material(&self) -> PyMaterial {
        PyMaterial(self.0.material())
    }

    #[getter]
    fn clipping(&self) -> PyClipSet {
        PyClipSet(self.0.clipping())
    }

    #[getter]
    fn face_visibility(&self) -> PyFaceVisibility {
        self.0.face_visibility().into()
    }

    #[getter]
    fn component_policy(&self) -> PySurfaceComponentPolicy {
        PySurfaceComponentPolicy(self.0.component_policy())
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible()
    }

    #[getter]
    fn bounds(&self) -> PyAabb {
        PyAabb(self.0.bounds())
    }

    #[getter]
    fn vertex_count(&self) -> usize {
        self.0.vertices().len()
    }

    #[getter]
    fn triangle_count(&self) -> usize {
        self.0.indices().len() / 3
    }

    /// One vertex by index, in stream order.
    fn vertex(&self, index: usize) -> Option<PyMeshVertex> {
        self.0.vertices().get(index).copied().map(PyMeshVertex)
    }

    /// Copies the model-space positions into one `(vertex_count, 3)` table.
    fn copy_positions_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f32>>> {
        let vertices = self.0.vertices();
        let rows = vertices.len();
        let table = PyArray2::zeros(py, (rows, 3), false);
        {
            let mut view = table.readwrite();
            let out = view
                .as_slice_mut()
                .map_err(|_| crate::error::value("vertex table must stay contiguous"))?;
            for (slot, vertex) in out.chunks_exact_mut(3).zip(vertices) {
                slot.copy_from_slice(&vertex.position.to_array());
            }
        }
        Ok(table)
    }

    /// Copies the model-space normals into one `(vertex_count, 3)` table.
    fn copy_normals_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f32>>> {
        let vertices = self.0.vertices();
        let rows = vertices.len();
        let table = PyArray2::zeros(py, (rows, 3), false);
        {
            let mut view = table.readwrite();
            let out = view
                .as_slice_mut()
                .map_err(|_| crate::error::value("vertex table must stay contiguous"))?;
            for (slot, vertex) in out.chunks_exact_mut(3).zip(vertices) {
                slot.copy_from_slice(&vertex.normal.to_array());
            }
        }
        Ok(table)
    }

    /// Copies the per-vertex colours into one `(vertex_count, 4)` table.
    fn copy_colors_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<u8>>> {
        let vertices = self.0.vertices();
        let rows = vertices.len();
        let table = PyArray2::zeros(py, (rows, 4), false);
        {
            let mut view = table.readwrite();
            let out = view
                .as_slice_mut()
                .map_err(|_| crate::error::value("vertex table must stay contiguous"))?;
            for (slot, vertex) in out.chunks_exact_mut(4).zip(vertices) {
                slot[0] = vertex.color.r;
                slot[1] = vertex.color.g;
                slot[2] = vertex.color.b;
                slot[3] = vertex.color.a;
            }
        }
        Ok(table)
    }

    /// Copies the triangle indices.
    fn copy_indices_numpy<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<u32>> {
        PyArray1::from_slice(py, self.0.indices())
    }
}

#[pymethods]
impl PyScene {
    /// Resolves a mesh by handle.
    fn mesh(&self, handle: PyMeshHandle) -> Option<PyMesh> {
        self.inner.mesh(handle.0).map(Into::into)
    }

    /// Iterates stored meshes in deterministic handle order.
    fn meshes(&self) -> Vec<(PyMeshHandle, PyMesh)> {
        self.inner
            .meshes()
            .map(|(handle, mesh)| (handle.into(), mesh.into()))
            .collect()
    }
}
