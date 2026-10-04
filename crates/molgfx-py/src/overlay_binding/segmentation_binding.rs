//! Categorical-grid authoring values and runtime label bindings.

use super::{PyDataSource, rgb};
use crate::binding::error;
use crate::scene_binding::PyScene;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::sync::Arc;
#[derive(Clone, Debug)]
#[pyclass(name = "Segmentation", frozen, skip_from_py_object)]
pub(super) struct PySegmentation(pub(super) molgfx::SegmentationSpec);

#[derive(Clone, Debug)]
#[pyclass(name = "SegmentStyle", frozen, skip_from_py_object)]
pub(super) struct PySegmentStyle(pub(super) molgfx::SegmentStyle);
#[pyfunction(name = "style")]
#[pyo3(signature = (label, *, color=(49,104,142), opacity=1.0, visible=true))]
fn segment_style(label: u32, color: (u8, u8, u8), opacity: f32, visible: bool) -> PySegmentStyle {
    PySegmentStyle(molgfx::SegmentStyle {
        label,
        color: rgb(color),
        opacity,
        visible,
    })
}

#[pyfunction(name = "volume")]
#[pyo3(signature = (*, source, dimensions, styles, voxel_to_world=None, presentation="surface"))]
fn segmentation(
    source: &PyDataSource,
    dimensions: (u32, u32, u32),
    styles: Vec<PyRef<'_, PySegmentStyle>>,
    voxel_to_world: Option<[f32; 16]>,
    presentation: &str,
) -> PyResult<PySegmentation> {
    let mut affine = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    if let Some(matrix) = voxel_to_world {
        affine = matrix;
    }
    Ok(PySegmentation(molgfx::SegmentationSpec {
        presentation: serde_json::from_value(serde_json::json!(presentation))
            .map_err(|cause| PyValueError::new_err(cause.to_string()))?,
        source: source.0.clone(),
        dimensions: [dimensions.0, dimensions.1, dimensions.2],
        voxel_to_world: affine,
        styles: styles.into_iter().map(|style| style.0).collect(),
    }))
}

#[pymethods]
impl PySegmentation {
    #[staticmethod]
    fn from_json(source: &str) -> PyResult<Self> {
        serde_json::from_str(source)
            .map(Self)
            .map_err(|cause| pyo3::exceptions::PyValueError::new_err(cause.to_string()))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.0)
            .map_err(|cause| pyo3::exceptions::PyValueError::new_err(cause.to_string()))
    }
}
#[pymethods]
impl PyScene {
    /// Binds exact unsigned labels to an authored categorical grid.
    fn bind_segmentation(
        &mut self,
        identity: &crate::id_binding::PySegmentationId,
        labels: Vec<u32>,
    ) -> PyResult<()> {
        let spec = self
            .inner
            .spec()
            .segmentations
            .get(&identity.0)
            .ok_or_else(|| PyValueError::new_err("unknown segmentation identity"))?;
        let binding = molgfx::SegmentationBinding::new(
            spec.source.clone(),
            spec.dimensions,
            spec.voxel_to_world,
            Arc::from(labels),
        )
        .map_err(error)?;
        self.inner.bind_segmentation(binding).map_err(error)
    }

    /// Replaces styles atomically and publishes the same portable patch.
    fn set_segment_styles(
        &mut self,
        py: Python<'_>,
        identity: &crate::id_binding::PySegmentationId,
        styles: Vec<PyRef<'_, PySegmentStyle>>,
    ) -> PyResult<()> {
        self.stage_or_apply(
            py,
            molgfx::schema::PatchOperation::SetSegmentStyles {
                id: identity.0,
                styles: styles.into_iter().map(|style| style.0).collect(),
            },
        )
    }

    fn remove_segmentation(
        &mut self,
        py: Python<'_>,
        identity: &crate::id_binding::PySegmentationId,
    ) -> PyResult<()> {
        self.stage_or_apply(
            py,
            molgfx::schema::PatchOperation::RemoveSegmentation { id: identity.0 },
        )
    }

    /// Resolves renderer provenance into the canonical semantic pick JSON.
    fn resolve_pick(&self, pick: &crate::render_binding::PyPickResult) -> PyResult<String> {
        let resolved = self.inner.resolve_pick(&pick.0).map_err(error)?;
        serde_json::to_string(&resolved).map_err(|cause| PyValueError::new_err(cause.to_string()))
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySegmentation>()?;
    module.add_class::<PySegmentStyle>()?;
    let segments = PyModule::new(module.py(), "segmentation")?;
    segments.add_function(wrap_pyfunction!(segmentation, &segments)?)?;
    segments.add_function(wrap_pyfunction!(segment_style, &segments)?)?;
    module.add_submodule(&segments)
}
