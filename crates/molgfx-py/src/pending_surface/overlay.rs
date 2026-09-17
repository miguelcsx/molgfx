//! Scene-independent screen-overlay value adapter.

use crate::error::core;
use crate::math::PyRgba8;
use crate::overlay_authoring::PyOverlayAnchor;
use pyo3::prelude::*;

#[pyclass(name = "ScreenOverlay", from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyScreenOverlay(pub(crate) molgfx::ScreenOverlay);

#[pymethods]
impl PyScreenOverlay {
    #[staticmethod]
    #[pyo3(signature = (text, anchor, color, size_pixels=14.0))]
    fn text(
        text: String,
        anchor: PyOverlayAnchor,
        color: PyRgba8,
        size_pixels: f32,
    ) -> PyResult<Self> {
        core(molgfx::ScreenOverlay::new(
            molgfx::OverlayContent::Text {
                text,
                color: color.0,
                size_pixels,
            },
            anchor.0,
        ))
        .map(Self)
    }

    #[staticmethod]
    fn coordinate_tripod(
        anchor: PyOverlayAnchor,
        size_pixels: f32,
        width_pixels: f32,
    ) -> PyResult<Self> {
        core(molgfx::ScreenOverlay::new(
            molgfx::OverlayContent::CoordinateTripod {
                size_pixels,
                width_pixels,
            },
            anchor.0,
        ))
        .map(Self)
    }

    #[getter]
    fn anchor(&self) -> PyOverlayAnchor {
        PyOverlayAnchor(self.0.anchor())
    }

    #[getter]
    fn order(&self) -> i16 {
        self.0.order()
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible()
    }

    fn set_anchor(&mut self, anchor: PyOverlayAnchor) {
        self.0.set_anchor(anchor.0);
    }

    fn set_order(&mut self, order: i16) {
        self.0.set_order(order);
    }

    fn set_visible(&mut self, visible: bool) {
        self.0.set_visible(visible);
    }
}
