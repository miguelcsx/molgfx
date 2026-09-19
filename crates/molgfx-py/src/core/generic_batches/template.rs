//! Shared analytic sphere/capsule templates.

use super::arrays::{contiguous, ordered_rows, validate_columns};
use crate::error::core;
use crate::math::PyVec3;
use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use std::sync::Arc;

/// One analytic sphere part of a template.
#[pyclass(name = "AnalyticSphere", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyAnalyticSphere(pub(crate) molgfx::core::AnalyticSphere);

#[pymethods]
impl PyAnalyticSphere {
    #[new]
    fn new(center: PyVec3, radius: f32) -> Self {
        Self(molgfx::core::AnalyticSphere {
            center: [center.0.x, center.0.y, center.0.z],
            radius,
        })
    }

    #[getter]
    fn center(&self) -> PyVec3 {
        PyVec3(molgfx::math::Vec3::from_array(self.0.center))
    }

    #[getter]
    fn radius(&self) -> f32 {
        self.0.radius
    }

    fn __repr__(&self) -> String {
        format!(
            "AnalyticSphere(center={}, radius={})",
            vec3(self.0.center),
            self.0.radius
        )
    }
}

/// The vector text shared by both parts, matching `Vec3`'s own `__repr__`.
fn vec3(values: [f32; 3]) -> String {
    format!("Vec3({}, {}, {})", values[0], values[1], values[2])
}

/// One analytic capsule part of a template.
#[pyclass(name = "AnalyticCapsule", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyAnalyticCapsule(pub(crate) molgfx::core::AnalyticCapsule);

#[pymethods]
impl PyAnalyticCapsule {
    #[new]
    fn new(start: PyVec3, end: PyVec3, radius: f32) -> PyResult<Self> {
        core(molgfx::core::AnalyticCapsule::new(start.0, end.0, radius)).map(Self)
    }

    #[getter]
    fn start(&self) -> PyVec3 {
        PyVec3(self.0.start())
    }

    #[getter]
    fn end(&self) -> PyVec3 {
        PyVec3(self.0.end())
    }

    #[getter]
    fn radius(&self) -> f32 {
        self.0.radius()
    }

    fn __repr__(&self) -> String {
        format!(
            "AnalyticCapsule(start={}, end={}, radius={})",
            vec3(self.0.start().to_array()),
            vec3(self.0.end().to_array()),
            self.0.radius()
        )
    }
}

#[pyclass(name = "AnalyticTemplate", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAnalyticTemplate(pub(super) Arc<molgfx::core::AnalyticTemplate>);

#[pymethods]
impl PyAnalyticTemplate {
    #[new]
    #[pyo3(signature = (namespace, spheres, capsules))]
    fn new(
        namespace: u64,
        spheres: PyReadonlyArray2<'_, f32>,
        capsules: PyReadonlyArray2<'_, f32>,
    ) -> PyResult<Self> {
        validate_columns("spheres", spheres.shape(), 4)?;
        validate_columns("capsules", capsules.shape(), 7)?;
        let sphere_values = contiguous("spheres", &spheres)?;
        let capsule_values = contiguous("capsules", &capsules)?;
        let spheres = sphere_values
            .chunks_exact(4)
            .map(|row| molgfx::core::AnalyticSphere {
                center: [row[0], row[1], row[2]],
                radius: row[3],
            })
            .collect::<Vec<_>>();
        let mut capsules = Vec::with_capacity(capsule_values.len() / 7);
        for row in capsule_values.chunks_exact(7) {
            capsules.push(core(molgfx::core::AnalyticCapsule::new(
                molgfx::math::Vec3::new(row[0], row[1], row[2]),
                molgfx::math::Vec3::new(row[3], row[4], row[5]),
                row[6],
            ))?);
        }
        let count = spheres.len().saturating_add(capsules.len());
        let rows = ordered_rows(namespace, count)?;
        core(molgfx::core::AnalyticTemplate::new(
            Arc::from(spheres),
            Arc::from(capsules),
            rows,
        ))
        .map(|template| Self(Arc::new(template)))
    }

    #[getter]
    fn part_count(&self) -> usize {
        self.0.part_count()
    }

    /// The sphere parts, in template order.
    #[getter]
    fn spheres(&self) -> Vec<PyAnalyticSphere> {
        self.0
            .spheres()
            .iter()
            .map(|sphere| PyAnalyticSphere(*sphere))
            .collect()
    }

    /// The capsule parts, in template order.
    #[getter]
    fn capsules(&self) -> Vec<PyAnalyticCapsule> {
        self.0
            .capsules()
            .iter()
            .map(|capsule| PyAnalyticCapsule(*capsule))
            .collect()
    }
}

impl PyAnalyticTemplate {
    pub(crate) fn native(&self) -> Arc<molgfx::core::AnalyticTemplate> {
        Arc::clone(&self.0)
    }
}
