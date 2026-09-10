//! Shared analytic sphere/capsule templates.

use super::arrays::{contiguous, ordered_rows, validate_columns};
use crate::error::core;
use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "AnalyticTemplate", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAnalyticTemplate(pub(super) Arc<pdviewx::AnalyticTemplate>);

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
            .map(|row| pdviewx::AnalyticSphere {
                center: [row[0], row[1], row[2]],
                radius: row[3],
            })
            .collect::<Vec<_>>();
        let mut capsules = Vec::with_capacity(capsule_values.len() / 7);
        for row in capsule_values.chunks_exact(7) {
            capsules.push(core(pdviewx::AnalyticCapsule::new(
                pdviewx::Vec3::new(row[0], row[1], row[2]),
                pdviewx::Vec3::new(row[3], row[4], row[5]),
                row[6],
            ))?);
        }
        let count = spheres.len().saturating_add(capsules.len());
        let rows = ordered_rows(namespace, count)?;
        core(pdviewx::AnalyticTemplate::new(
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
}

impl PyAnalyticTemplate {
    pub(crate) fn native(&self) -> Arc<pdviewx::AnalyticTemplate> {
        Arc::clone(&self.0)
    }
}
