//! Typed, concise screen-overlay authoring for Python.

use crate::core::{PyOverlayHandle, PyScene};
use crate::error::core;
use crate::math::PyRgba8;
use pyo3::prelude::*;

#[pyclass(name = "OverlayAnchor", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyOverlayAnchor(pub(crate) molgfx::OverlayAnchor);

#[pymethods]
impl PyOverlayAnchor {
    #[new]
    #[pyo3(signature = (normalized, pixels=(0.0, 0.0)))]
    fn new(normalized: (f32, f32), pixels: (f32, f32)) -> PyResult<Self> {
        core(molgfx::OverlayAnchor::new(
            [normalized.0, normalized.1],
            [pixels.0, pixels.1],
        ))
        .map(Self)
    }

    #[getter]
    fn normalized(&self) -> (f32, f32) {
        self.0.normalized.into()
    }

    #[getter]
    fn pixels(&self) -> (f32, f32) {
        self.0.pixels.into()
    }
}

#[pymethods]
impl PyScene {
    #[pyo3(signature = (text, anchor, color, size_pixels=14.0, order=0))]
    fn add_text_overlay(
        &mut self,
        text: String,
        anchor: PyOverlayAnchor,
        color: PyRgba8,
        size_pixels: f32,
        order: i16,
    ) -> PyResult<PyOverlayHandle> {
        self.add_overlay(
            molgfx::OverlayContent::Text {
                text,
                color: color.0,
                size_pixels,
            },
            anchor.0,
            order,
        )
    }

    #[pyo3(signature = (title, range, colors, size_pixels, anchor, order=0))]
    fn add_color_legend(
        &mut self,
        title: String,
        range: (f32, f32),
        colors: (PyRgba8, PyRgba8),
        size_pixels: (f32, f32),
        anchor: PyOverlayAnchor,
        order: i16,
    ) -> PyResult<PyOverlayHandle> {
        self.add_overlay(
            molgfx::OverlayContent::ColorLegend {
                title,
                range: [range.0, range.1],
                colors: [colors.0 .0, colors.1 .0],
                size_pixels: [size_pixels.0, size_pixels.1],
            },
            anchor.0,
            order,
        )
    }

    #[pyo3(signature = (length_angstrom, anchor, color, width_pixels=2.0, order=0))]
    fn add_scale_bar(
        &mut self,
        length_angstrom: f32,
        anchor: PyOverlayAnchor,
        color: PyRgba8,
        width_pixels: f32,
        order: i16,
    ) -> PyResult<PyOverlayHandle> {
        self.add_overlay(
            molgfx::OverlayContent::ScaleBar {
                length_angstrom,
                color: color.0,
                width_pixels,
            },
            anchor.0,
            order,
        )
    }

    #[pyo3(signature = (anchor, size_pixels=48.0, width_pixels=2.0, order=0))]
    fn add_coordinate_tripod(
        &mut self,
        anchor: PyOverlayAnchor,
        size_pixels: f32,
        width_pixels: f32,
        order: i16,
    ) -> PyResult<PyOverlayHandle> {
        self.add_overlay(
            molgfx::OverlayContent::CoordinateTripod {
                size_pixels,
                width_pixels,
            },
            anchor.0,
            order,
        )
    }

    fn set_overlay_visible(&mut self, handle: PyOverlayHandle, visible: bool) -> bool {
        let Some(overlay) = self.inner.overlay_mut(handle.0) else {
            return false;
        };
        overlay.set_visible(visible);
        true
    }

    fn remove_overlay(&mut self, handle: PyOverlayHandle) -> bool {
        self.inner.remove_overlay(handle.0).is_some()
    }
}

impl PyScene {
    fn add_overlay(
        &mut self,
        content: molgfx::OverlayContent,
        anchor: molgfx::OverlayAnchor,
        order: i16,
    ) -> PyResult<PyOverlayHandle> {
        let mut overlay = core(molgfx::ScreenOverlay::new(content, anchor))?;
        overlay.set_order(order);
        Ok(self.inner.add_overlay(overlay).into())
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyOverlayAnchor>()
}
