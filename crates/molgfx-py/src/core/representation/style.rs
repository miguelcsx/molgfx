//! Stroke style of a generic relation glyph.

use crate::core::PyRelationPattern;
use crate::math::PyRgba8;
use pyo3::prelude::*;

#[pyclass(name = "RelationStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRelationStyle(pub(crate) molgfx::core::RelationStyle);

#[pymethods]
impl PyRelationStyle {
    #[new]
    #[pyo3(signature = (width_pixels=1.5, color=None, opacity=1.0, pattern=None, endpoint_insets_pixels=(0.0, 0.0), depth_behind_anchors=false))]
    fn new(
        width_pixels: f32,
        color: Option<PyRgba8>,
        opacity: f32,
        pattern: Option<PyRelationPattern>,
        endpoint_insets_pixels: (f32, f32),
        depth_behind_anchors: bool,
    ) -> Self {
        let default = molgfx::core::RelationStyle::default();
        Self(molgfx::core::RelationStyle {
            width_pixels,
            color: color.map_or(default.color, |color| color.0),
            opacity,
            pattern: pattern.map_or(default.pattern, Into::into),
            endpoint_insets_pixels: [endpoint_insets_pixels.0, endpoint_insets_pixels.1],
            depth_behind_anchors,
        })
    }

    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::core::RelationStyle::default())
    }

    #[getter]
    fn width_pixels(&self) -> f32 {
        self.0.width_pixels
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        PyRgba8(self.0.color)
    }

    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }

    #[getter]
    fn pattern(&self) -> PyRelationPattern {
        self.0.pattern.into()
    }

    #[getter]
    fn endpoint_insets_pixels(&self) -> (f32, f32) {
        let [start, end] = self.0.endpoint_insets_pixels;
        (start, end)
    }

    #[getter]
    fn depth_behind_anchors(&self) -> bool {
        self.0.depth_behind_anchors
    }
}
