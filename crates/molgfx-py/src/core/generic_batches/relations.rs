//! Relation records and world/entity insertion from contiguous `NumPy` columns.

use super::super::{PyRelationBatchHandle, PyRelationStyle, PyScene};
use super::anchors::{PyRelationLayout, PySpatialAnchor};
use super::arrays::{ordered_rows, vec3_rows};
use super::model::PyRowDomain;
use crate::error::{core, value};
use numpy::{PyArray1, PyReadonlyArray1, PyReadonlyArray2};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "Relation", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRelation(pub(crate) molgfx::core::Relation);

impl From<molgfx::core::Relation> for PyRelation {
    fn from(value: molgfx::core::Relation) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyRelation {
    #[new]
    fn new(start: PySpatialAnchor, end: PySpatialAnchor) -> Self {
        Self(molgfx::core::Relation {
            start: start.0,
            end: end.0,
        })
    }

    #[getter]
    fn start(&self) -> PySpatialAnchor {
        PySpatialAnchor(self.0.start)
    }

    #[getter]
    fn end(&self) -> PySpatialAnchor {
        PySpatialAnchor(self.0.end)
    }

    /// Ordered homogeneous layout.
    #[getter]
    fn layout(&self) -> PyRelationLayout {
        PyRelationLayout(self.0.layout())
    }
}

/// One layout-homogeneous run of the partitioned relation stream.
#[pyclass(name = "RelationPartition", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRelationPartition(pub(crate) molgfx::core::RelationPartition);

impl From<&molgfx::core::RelationPartition> for PyRelationPartition {
    fn from(value: &molgfx::core::RelationPartition) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyRelationPartition {
    /// Branch-free endpoint layout of the run.
    #[getter]
    fn layout(&self) -> PyRelationLayout {
        PyRelationLayout(self.0.layout)
    }

    /// First and last rows of the run in partitioned stream order.
    #[getter]
    fn rows(&self) -> (u32, u32) {
        (self.0.rows.start, self.0.rows.end)
    }
}

/// Highest referenced row for one external spatial domain.
#[pyclass(name = "RelationDependency", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRelationDependency(pub(crate) molgfx::core::RelationDependency);

impl From<molgfx::core::RelationDependency> for PyRelationDependency {
    fn from(value: molgfx::core::RelationDependency) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyRelationDependency {
    #[getter]
    fn domain(&self) -> PyRowDomain {
        PyRowDomain(self.0.domain)
    }

    #[getter]
    fn maximum_row(&self) -> u32 {
        self.0.maximum_row
    }
}

/// One stored relation batch with its precomputed partition.
#[pyclass(name = "RelationBatch", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRelationBatch(pub(crate) molgfx::core::RelationBatch);

impl From<&molgfx::core::RelationBatch> for PyRelationBatch {
    fn from(value: &molgfx::core::RelationBatch) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyRelationBatch {
    /// Logical relations in caller order.
    #[getter]
    fn relations(&self) -> Vec<PyRelation> {
        self.0
            .relations()
            .iter()
            .map(|relation| PyRelation(*relation))
            .collect()
    }

    /// Batch-wide fallback style.
    #[getter]
    fn style(&self) -> PyRelationStyle {
        PyRelationStyle(self.0.style())
    }

    /// Homogeneous stream partitions in deterministic layout order.
    #[getter]
    fn partitions(&self) -> Vec<PyRelationPartition> {
        self.0
            .partitions()
            .iter()
            .map(PyRelationPartition::from)
            .collect()
    }

    /// Deduplicated external domains the batch references.
    #[getter]
    fn dependencies(&self) -> Vec<PyRelationDependency> {
        self.0
            .dependencies()
            .iter()
            .map(|dependency| PyRelationDependency(*dependency))
            .collect()
    }

    /// Current scene visibility.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible()
    }

    /// Number of logical relation rows.
    #[getter]
    fn row_count(&self) -> u32 {
        self.0.source_rows().len()
    }

    /// Copies the partitioned-row to logical-row mapping, absent when the
    /// grouping already keeps caller order.
    fn copy_remap_numpy<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<u32>>> {
        self.0
            .remap()
            .map(|remap| PyArray1::from_slice(py, remap.as_ref()))
    }
}

#[pymethods]
impl PyScene {
    #[pyo3(signature = (namespace, starts, ends, style=None))]
    fn add_world_relations_from_numpy(
        &mut self,
        namespace: u64,
        starts: PyReadonlyArray2<'_, f32>,
        ends: PyReadonlyArray2<'_, f32>,
        style: Option<PyRelationStyle>,
    ) -> PyResult<PyRelationBatchHandle> {
        let starts = vec3_rows("starts", &starts)?;
        let ends = vec3_rows("ends", &ends)?;
        if starts.len() != ends.len() {
            return Err(value(
                "relation endpoint columns must have the same row count",
            ));
        }
        let mut relations = Vec::with_capacity(starts.len());
        for (start, end) in starts.into_iter().zip(ends) {
            relations.push(molgfx::core::Relation {
                start: core(molgfx::core::SpatialAnchor::world(
                    molgfx::math::Vec3::from_array(start),
                ))?,
                end: core(molgfx::core::SpatialAnchor::world(
                    molgfx::math::Vec3::from_array(end),
                ))?,
            });
        }
        self.insert_relations(namespace, relations, style)
    }

    #[pyo3(signature = (namespace, start_domain, start_rows, end_domain, end_rows, style=None))]
    fn add_entity_relations_from_numpy(
        &mut self,
        namespace: u64,
        start_domain: PyRowDomain,
        start_rows: PyReadonlyArray1<'_, u32>,
        end_domain: PyRowDomain,
        end_rows: PyReadonlyArray1<'_, u32>,
        style: Option<PyRelationStyle>,
    ) -> PyResult<PyRelationBatchHandle> {
        let start_rows = start_rows
            .as_slice()
            .map_err(|_| value("start_rows must be C-contiguous uint32"))?;
        let end_rows = end_rows
            .as_slice()
            .map_err(|_| value("end_rows must be C-contiguous uint32"))?;
        if start_rows.len() != end_rows.len() {
            return Err(value("relation row columns must have the same row count"));
        }
        let mut relations = Vec::with_capacity(start_rows.len());
        for (&start, &end) in start_rows.iter().zip(end_rows) {
            relations.push(molgfx::core::Relation {
                start: core(molgfx::core::SpatialAnchor::entity(
                    molgfx::core::RowEntityRef::new(start_domain.0, start),
                ))?,
                end: core(molgfx::core::SpatialAnchor::entity(
                    molgfx::core::RowEntityRef::new(end_domain.0, end),
                ))?,
            });
        }
        self.insert_relations(namespace, relations, style)
    }
}

#[pymethods]
impl PyScene {
    /// Resolves a relation batch by generational handle.
    fn relation_batch(&self, handle: PyRelationBatchHandle) -> Option<PyRelationBatch> {
        self.inner.relation_batch(handle.0).map(Into::into)
    }

    /// Iterates relation batches in stable slot order.
    fn relation_batches(&self) -> Vec<(PyRelationBatchHandle, PyRelationBatch)> {
        self.inner
            .relation_batches()
            .map(|(handle, batch)| (handle.into(), batch.into()))
            .collect()
    }

    /// Removes one relation batch and invalidates its handle.
    fn remove_relation_batch(&mut self, handle: PyRelationBatchHandle) -> Option<PyRelationBatch> {
        self.inner
            .remove_relation_batch(handle.0)
            .map(PyRelationBatch)
    }
}

impl PyScene {
    fn insert_relations(
        &mut self,
        namespace: u64,
        relations: Vec<molgfx::core::Relation>,
        style: Option<PyRelationStyle>,
    ) -> PyResult<PyRelationBatchHandle> {
        let rows = ordered_rows(namespace, relations.len())?;
        let style = style.map_or_else(molgfx::core::RelationStyle::default, |style| style.0);
        core(molgfx::core::RelationBatch::new(
            Arc::from(relations),
            rows,
            style,
        ))
        .and_then(|batch| core(self.inner.add_relation_batch(batch)))
        .map(Into::into)
    }
}
