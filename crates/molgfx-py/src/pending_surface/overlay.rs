//! Scene-independent screen-overlay value adapter.

use crate::error::core;
use crate::math::PyRgba8;
use crate::overlay_authoring::PyOverlayAnchor;
use pyo3::prelude::*;

/// Which variant of overlay content an overlay carries.
#[pyclass(name = "OverlayKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyOverlayKind {
    Text,
    ColorLegend,
    ScaleBar,
    CoordinateTripod,
}

/// Typed payload one screen overlay draws.
#[pyclass(name = "OverlayContent", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyOverlayContent(pub(crate) molgfx::core::OverlayContent);

impl From<&molgfx::core::OverlayContent> for PyOverlayContent {
    fn from(value: &molgfx::core::OverlayContent) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyOverlayContent {
    /// Which variant this payload carries.
    #[getter]
    fn kind(&self) -> PyOverlayKind {
        match self.0 {
            molgfx::core::OverlayContent::Text { .. } => PyOverlayKind::Text,
            molgfx::core::OverlayContent::ColorLegend { .. } => PyOverlayKind::ColorLegend,
            molgfx::core::OverlayContent::ScaleBar { .. } => PyOverlayKind::ScaleBar,
            molgfx::core::OverlayContent::CoordinateTripod { .. } => {
                PyOverlayKind::CoordinateTripod
            }
        }
    }

    /// Caller-authored text, absent for the other variants.
    #[getter]
    fn text(&self) -> Option<String> {
        match &self.0 {
            molgfx::core::OverlayContent::Text { text, .. } => Some(text.clone()),
            _ => None,
        }
    }

    /// Legend title, absent for the other variants.
    #[getter]
    fn title(&self) -> Option<String> {
        match &self.0 {
            molgfx::core::OverlayContent::ColorLegend { title, .. } => Some(title.clone()),
            _ => None,
        }
    }

    /// Colour of a text or scale bar, absent for the other variants.
    #[getter]
    fn color(&self) -> Option<PyRgba8> {
        match &self.0 {
            molgfx::core::OverlayContent::Text { color, .. }
            | molgfx::core::OverlayContent::ScaleBar { color, .. } => Some(PyRgba8(*color)),
            _ => None,
        }
    }

    /// Endpoint colours of a legend, absent for the other variants.
    #[getter]
    fn colors(&self) -> Option<(PyRgba8, PyRgba8)> {
        match &self.0 {
            molgfx::core::OverlayContent::ColorLegend { colors, .. } => {
                Some((PyRgba8(colors[0]), PyRgba8(colors[1])))
            }
            _ => None,
        }
    }

    /// Inclusive scalar endpoints of a legend, absent for the other variants.
    #[getter]
    fn range(&self) -> Option<(f32, f32)> {
        match &self.0 {
            molgfx::core::OverlayContent::ColorLegend { range, .. } => Some((range[0], range[1])),
            _ => None,
        }
    }

    /// Size in physical pixels: the rectangle of a legend, or the scalar of a
    /// text cap height or tripod axis repeated for both axes.
    #[getter]
    fn size_pixels(&self) -> Option<(f32, f32)> {
        match &self.0 {
            molgfx::core::OverlayContent::Text { size_pixels, .. }
            | molgfx::core::OverlayContent::CoordinateTripod { size_pixels, .. } => {
                Some((*size_pixels, *size_pixels))
            }
            molgfx::core::OverlayContent::ColorLegend { size_pixels, .. } => {
                Some((size_pixels[0], size_pixels[1]))
            }
            molgfx::core::OverlayContent::ScaleBar { .. } => None,
        }
    }

    /// Represented world length of a scale bar in ångström, absent for the
    /// other variants.
    #[getter]
    fn length_angstrom(&self) -> Option<f32> {
        match &self.0 {
            molgfx::core::OverlayContent::ScaleBar {
                length_angstrom, ..
            } => Some(*length_angstrom),
            _ => None,
        }
    }

    /// Line width in physical pixels of a scale bar or tripod, absent for the
    /// other variants.
    #[getter]
    fn width_pixels(&self) -> Option<f32> {
        match &self.0 {
            molgfx::core::OverlayContent::ScaleBar { width_pixels, .. }
            | molgfx::core::OverlayContent::CoordinateTripod { width_pixels, .. } => {
                Some(*width_pixels)
            }
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!("OverlayContent.{:?}", self.kind())
    }
}

#[pyclass(name = "ScreenOverlay", from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyScreenOverlay(pub(crate) molgfx::core::ScreenOverlay);

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
        core(molgfx::core::ScreenOverlay::new(
            molgfx::core::OverlayContent::Text {
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
        core(molgfx::core::ScreenOverlay::new(
            molgfx::core::OverlayContent::CoordinateTripod {
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

    /// Typed payload this overlay draws.
    #[getter]
    fn content(&self) -> PyOverlayContent {
        self.0.content().into()
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
