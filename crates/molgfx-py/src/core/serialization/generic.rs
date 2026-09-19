//! Generic row-table records and the visuals attached to their domains.
//!
//! A generic batch keeps its immutable data outside the manifest — positions,
//! keys, templates — and records only a content address for it. The fallback
//! values here are what a batch draws with before that payload is resolved.

use super::identity::{PyRowDomainDescription, PySourceRowsDescription};
use super::manifest::PyPayloadReference;
use super::representation::PyVisualStyleDescription;
use pyo3::prelude::*;

/// Generic 12-byte point source and batch-wide fallback style.
#[pyclass(name = "PointBatchDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyPointBatchDescription(pub(crate) molgfx::core::PointBatchDescription);

#[pymethods]
impl PyPointBatchDescription {
    /// Stable point-batch slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Stable point-batch slot generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// External identity of logical point rows.
    #[getter]
    fn source_rows(&self) -> PySourceRowsDescription {
        PySourceRowsDescription(self.0.source_rows)
    }

    /// `disc` or `sphere`.
    #[getter]
    fn glyph(&self) -> String {
        self.0.glyph.clone()
    }

    /// Exact bits of the positive fallback radius.
    #[getter]
    fn radius_bits(&self) -> u32 {
        self.0.radius_bits
    }

    /// Packed fallback color.
    #[getter]
    fn color(&self) -> [u8; 4] {
        self.0.color
    }

    /// Whether the batch draws.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }

    /// Content-addressed positions, keys and immutable batch metadata.
    #[getter]
    fn payload(&self) -> PyPayloadReference {
        PyPayloadReference(self.0.payload)
    }
}

/// Shared-template rigid-instance source.
#[pyclass(name = "InstanceBatchDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyInstanceBatchDescription(pub(crate) molgfx::core::InstanceBatchDescription);

#[pymethods]
impl PyInstanceBatchDescription {
    /// Stable instance-batch slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Stable instance-batch slot generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// External identity of rigid-instance rows.
    #[getter]
    fn source_rows(&self) -> PySourceRowsDescription {
        PySourceRowsDescription(self.0.source_rows)
    }

    /// External identity of flattened analytic-template parts.
    #[getter]
    fn template_rows(&self) -> PySourceRowsDescription {
        PySourceRowsDescription(self.0.template_rows)
    }

    /// Number of homogeneous sphere parts.
    #[getter]
    fn sphere_count(&self) -> u32 {
        self.0.sphere_count
    }

    /// Number of homogeneous capsule parts.
    #[getter]
    fn capsule_count(&self) -> u32 {
        self.0.capsule_count
    }

    /// Packed fallback color.
    #[getter]
    fn color(&self) -> [u8; 4] {
        self.0.color
    }

    /// Whether the batch draws.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }

    /// Content-addressed template, transforms and both source-key columns.
    #[getter]
    fn payload(&self) -> PyPayloadReference {
        PyPayloadReference(self.0.payload)
    }
}

/// One immutable typed column retained outside JSON.
#[pyclass(name = "AttributeDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAttributeDescription(pub(crate) molgfx::core::AttributeDescription);

#[pymethods]
impl PyAttributeDescription {
    /// Stable attribute slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Stable attribute slot generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// Exact target row table.
    #[getter]
    fn domain(&self) -> PyRowDomainDescription {
        PyRowDomainDescription(self.0.domain.clone())
    }

    /// Caller-authored introspection name.
    #[getter]
    fn name(&self) -> String {
        self.0.name.clone()
    }

    /// Optional uninterpreted measured quantity.
    #[getter]
    fn quantity(&self) -> Option<String> {
        self.0.quantity.clone()
    }

    /// Optional caller unit string.
    #[getter]
    fn unit(&self) -> Option<String> {
        self.0.unit.clone()
    }

    /// Optional caller method, dataset or evidence identifier.
    #[getter]
    fn provenance(&self) -> Option<String> {
        self.0.provenance.clone()
    }

    /// `scalar`, `category`, `vector` or `color`.
    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    /// Number of physical rows.
    #[getter]
    fn row_count(&self) -> u32 {
        self.0.row_count
    }

    /// Deterministic typed-column fingerprint.
    #[getter]
    fn fingerprint(&self) -> u64 {
        self.0.fingerprint
    }

    /// Content-addressed native-width values.
    #[getter]
    fn payload(&self) -> PyPayloadReference {
        PyPayloadReference(self.0.payload)
    }
}

/// Generic spatial relation source and visual fallback.
#[pyclass(name = "RelationBatchDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRelationBatchDescription(pub(crate) molgfx::core::RelationBatchDescription);

#[pymethods]
impl PyRelationBatchDescription {
    /// Stable relation-batch slot row.
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    /// Stable relation-batch slot generation.
    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    /// External identity of logical relation rows.
    #[getter]
    fn source_rows(&self) -> PySourceRowsDescription {
        PySourceRowsDescription(self.0.source_rows)
    }

    /// Exact bits of the positive fallback width.
    #[getter]
    fn width_bits(&self) -> u32 {
        self.0.width_bits
    }

    /// Packed fallback color.
    #[getter]
    fn color(&self) -> [u8; 4] {
        self.0.color
    }

    /// Exact bits of the bounded fallback opacity.
    #[getter]
    fn opacity_bits(&self) -> u32 {
        self.0.opacity_bits
    }

    /// Exact bits of the non-negative start and end screen-space trims.
    #[getter]
    fn endpoint_inset_bits(&self) -> [u32; 2] {
        self.0.endpoint_inset_bits
    }

    /// Whether opaque scene geometry occludes the connector.
    #[getter]
    fn depth_behind_anchors(&self) -> bool {
        self.0.depth_behind_anchors
    }

    /// `solid`, `dashed` or `dotted`.
    #[getter]
    fn pattern(&self) -> String {
        self.0.pattern.clone()
    }

    /// Whether the batch draws.
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }

    /// Content-addressed anchors, keys and immutable batch metadata.
    #[getter]
    fn payload(&self) -> PyPayloadReference {
        PyPayloadReference(self.0.payload)
    }
}

/// Visual program and parameters attached directly to one generic domain.
#[pyclass(name = "DomainVisualDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyDomainVisualDescription(pub(crate) molgfx::core::DomainVisualDescription);

#[pymethods]
impl PyDomainVisualDescription {
    /// Exact row table receiving the visual program.
    #[getter]
    fn domain(&self) -> PyRowDomainDescription {
        PyRowDomainDescription(self.0.domain.clone())
    }

    /// Stable caller-defined draw and composition order.
    #[getter]
    fn order(&self) -> i32 {
        self.0.order
    }

    /// Typed bounded program and its parameter values.
    #[getter]
    fn style(&self) -> PyVisualStyleDescription {
        PyVisualStyleDescription(self.0.style.clone())
    }
}
