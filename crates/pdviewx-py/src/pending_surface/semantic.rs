//! Typed semantic-composition adapters.

use crate::core::{
    PyRepresentationHandle, PyRepresentationKind, PyScene, PySelectionHandle, PyStructureHandle,
};
use crate::error::core;
use crate::math::{PyRgba8, PyVec3};
use crate::semantic::PyLodClusterKey;
use crate::values::{PySurfaceKind, PySurfaceStyle};
use pyo3::prelude::*;

#[pyclass(name = "LodCluster", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyLodCluster(pub(crate) pdviewx::LodCluster);

#[pymethods]
impl PyLodCluster {
    #[new]
    fn new(
        structure: PyStructureHandle,
        level: crate::semantic::PyLodLevel,
        index: u32,
        center: PyVec3,
        radius: f32,
        importance: f32,
        atom_count: u32,
    ) -> Self {
        Self(pdviewx::LodCluster {
            key: pdviewx::LodClusterKey {
                structure: structure.0,
                level: level.into(),
                index,
            },
            center: center.0,
            radius,
            importance,
            atom_count,
        })
    }

    #[getter]
    fn key(&self) -> PyLodClusterKey {
        PyLodClusterKey(self.0.key)
    }

    #[getter]
    fn center(&self) -> PyVec3 {
        PyVec3(self.0.center)
    }

    #[getter]
    fn radius(&self) -> f32 {
        self.0.radius
    }

    #[getter]
    fn importance(&self) -> f32 {
        self.0.importance
    }

    #[getter]
    fn atom_count(&self) -> u32 {
        self.0.atom_count
    }
}

#[pyclass(name = "SurfaceZone", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySurfaceZone(pub(crate) pdviewx::SurfaceZone);

#[pymethods]
impl PySurfaceZone {
    #[getter]
    fn selection(&self) -> PySelectionHandle {
        self.0.selection.into()
    }

    #[getter]
    fn representation(&self) -> PyRepresentationHandle {
        self.0.representation.into()
    }
}

#[pyclass(name = "SurfaceZoneStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySurfaceZoneStyle(pub(crate) pdviewx::SurfaceZoneStyle);

#[pymethods]
impl PySurfaceZoneStyle {
    #[new]
    #[pyo3(signature = (distance=5.0, kind=PySurfaceKind::SolventExcluded, presentation=PySurfaceStyle::Solid, opacity=0.35, color=None))]
    fn new(
        distance: f32,
        kind: PySurfaceKind,
        presentation: PySurfaceStyle,
        opacity: f32,
        color: Option<PyRgba8>,
    ) -> Self {
        let default = pdviewx::SurfaceZoneStyle::default();
        Self(pdviewx::SurfaceZoneStyle {
            distance,
            kind: kind.into(),
            presentation: presentation.into(),
            opacity,
            color: color.map_or(default.color, |value| value.0),
        })
    }

    #[staticmethod]
    fn default() -> Self {
        Self(pdviewx::SurfaceZoneStyle::default())
    }
}

#[pyclass(name = "SurfaceZoneScene", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PySurfaceZoneScene;

#[pymethods]
impl PySurfaceZoneScene {
    #[staticmethod]
    fn surface_zone(
        mut scene: PyRefMut<'_, PyScene>,
        surface: PySelectionHandle,
        around: PySelectionHandle,
    ) -> PyResult<PySurfaceZone> {
        core(pdviewx::SurfaceZoneScene::surface_zone(
            &mut scene.inner,
            surface.0,
            around.0,
        ))
        .map(PySurfaceZone)
    }

    #[staticmethod]
    fn surface_zone_with(
        mut scene: PyRefMut<'_, PyScene>,
        surface: PySelectionHandle,
        around: PySelectionHandle,
        style: PySurfaceZoneStyle,
    ) -> PyResult<PySurfaceZone> {
        core(pdviewx::SurfaceZoneScene::surface_zone_with(
            &mut scene.inner,
            surface.0,
            around.0,
            style.0,
        ))
        .map(PySurfaceZone)
    }

    #[staticmethod]
    fn surface_components(
        mut scene: PyRefMut<'_, PyScene>,
        surface: PySelectionHandle,
        minimum_atoms: u32,
    ) -> PyResult<PySurfaceZone> {
        core(pdviewx::SurfaceZoneScene::surface_components(
            &mut scene.inner,
            surface.0,
            minimum_atoms,
        ))
        .map(PySurfaceZone)
    }
}

const _: fn(PyRepresentationKind) = |_| {};
