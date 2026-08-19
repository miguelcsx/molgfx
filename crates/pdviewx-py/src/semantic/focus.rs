//! Python adapters for focus-and-context composition.

use crate::core::{PyRepresentationHandle, PyScene, PySelectionHandle};
use crate::error::{core, semantic};
use crate::math::PyRgba8;
use crate::values::PySurfaceStyle;
use pyo3::prelude::*;

#[pyclass(name = "FocusBand", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyFocusBand {
    Near,
    Mid,
    Far,
}

#[pyclass(name = "FocusContext", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyFocusContext {
    Cartoon,
    Trace,
    Tube,
}

#[pyclass(name = "FocusSurfaceExtent", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyFocusSurfaceExtent {
    Pocket,
    Structure,
}

impl From<PyFocusContext> for pdviewx::FocusContext {
    fn from(value: PyFocusContext) -> Self {
        match value {
            PyFocusContext::Cartoon => Self::Cartoon,
            PyFocusContext::Trace => Self::Trace,
            PyFocusContext::Tube => Self::Tube,
        }
    }
}

impl From<PyFocusSurfaceExtent> for pdviewx::FocusSurfaceExtent {
    fn from(value: PyFocusSurfaceExtent) -> Self {
        match value {
            PyFocusSurfaceExtent::Pocket => Self::Pocket,
            PyFocusSurfaceExtent::Structure => Self::Structure,
        }
    }
}

#[pyclass(name = "DistanceBands", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDistanceBands(pub(crate) pdviewx::DistanceBands);

#[pymethods]
impl PyDistanceBands {
    #[new]
    fn new(near: f32, mid: f32) -> PyResult<Self> {
        semantic(pdviewx::DistanceBands::new(near, mid)).map(Self)
    }

    #[staticmethod]
    fn default() -> Self {
        Self(pdviewx::DistanceBands::default())
    }

    #[getter]
    fn near(&self) -> f32 {
        self.0.near()
    }

    #[getter]
    fn mid(&self) -> f32 {
        self.0.mid()
    }

    fn classify(&self, distance: f32) -> PyFocusBand {
        match self.0.classify(distance) {
            pdviewx::FocusBand::Near => PyFocusBand::Near,
            pdviewx::FocusBand::Mid => PyFocusBand::Mid,
            pdviewx::FocusBand::Far => PyFocusBand::Far,
        }
    }
}

#[pyclass(name = "FocusStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFocusStyle(pub(crate) pdviewx::FocusStyle);

#[pymethods]
impl PyFocusStyle {
    #[new]
    #[pyo3(signature = (bands=None, pocket_opacity=0.34, context_opacity=0.30, pocket_presentation=None, surface_extent=None, pocket_color=None, context_geometry=None, context_color=None, solvent_opacity=0.42, solvent_color=None))]
    fn new(
        bands: Option<PyDistanceBands>,
        pocket_opacity: f32,
        context_opacity: f32,
        pocket_presentation: Option<PySurfaceStyle>,
        surface_extent: Option<PyFocusSurfaceExtent>,
        pocket_color: Option<PyRgba8>,
        context_geometry: Option<PyFocusContext>,
        context_color: Option<PyRgba8>,
        solvent_opacity: f32,
        solvent_color: Option<PyRgba8>,
    ) -> Self {
        let default = pdviewx::FocusStyle::default();
        Self(pdviewx::FocusStyle {
            bands: bands.map_or(default.bands, |value| value.0),
            pocket_opacity,
            context_opacity,
            pocket_presentation: pocket_presentation
                .map_or(default.pocket_presentation, Into::into),
            surface_extent: surface_extent.map_or(default.surface_extent, Into::into),
            pocket_color: pocket_color.map_or(default.pocket_color, |value| value.0),
            context_geometry: context_geometry.map_or(default.context_geometry, Into::into),
            context_color: context_color.map_or(default.context_color, |value| value.0),
            solvent_opacity,
            solvent_color: solvent_color.map_or(default.solvent_color, |value| value.0),
        })
    }

    #[staticmethod]
    fn default() -> Self {
        Self(pdviewx::FocusStyle::default())
    }

    #[getter]
    fn bands(&self) -> PyDistanceBands {
        PyDistanceBands(self.0.bands)
    }
}

#[pyclass(name = "FocusView", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFocusView {
    focus: PySelectionHandle,
    near: PySelectionHandle,
    pocket: PySelectionHandle,
    mid: PySelectionHandle,
    context: PySelectionHandle,
    solvent: PySelectionHandle,
    focus_representation: PyRepresentationHandle,
    near_representation: PyRepresentationHandle,
    mid_representation: PyRepresentationHandle,
    pocket_representation: PyRepresentationHandle,
    context_representation: PyRepresentationHandle,
    solvent_representation: PyRepresentationHandle,
}

impl From<pdviewx::FocusView> for PyFocusView {
    fn from(value: pdviewx::FocusView) -> Self {
        Self {
            focus: value.focus.into(),
            near: value.near.into(),
            pocket: value.pocket.into(),
            mid: value.mid.into(),
            context: value.context.into(),
            solvent: value.solvent.into(),
            focus_representation: value.focus_representation.into(),
            near_representation: value.near_representation.into(),
            mid_representation: value.mid_representation.into(),
            pocket_representation: value.pocket_representation.into(),
            context_representation: value.context_representation.into(),
            solvent_representation: value.solvent_representation.into(),
        }
    }
}

#[pymethods]
impl PyFocusView {
    #[getter]
    fn focus(&self) -> PySelectionHandle {
        self.focus
    }
    #[getter]
    fn near(&self) -> PySelectionHandle {
        self.near
    }
    #[getter]
    fn pocket(&self) -> PySelectionHandle {
        self.pocket
    }
    #[getter]
    fn mid(&self) -> PySelectionHandle {
        self.mid
    }
    #[getter]
    fn context(&self) -> PySelectionHandle {
        self.context
    }
    #[getter]
    fn solvent(&self) -> PySelectionHandle {
        self.solvent
    }
    #[getter]
    fn focus_representation(&self) -> PyRepresentationHandle {
        self.focus_representation
    }
    #[getter]
    fn near_representation(&self) -> PyRepresentationHandle {
        self.near_representation
    }
    #[getter]
    fn mid_representation(&self) -> PyRepresentationHandle {
        self.mid_representation
    }
    #[getter]
    fn pocket_representation(&self) -> PyRepresentationHandle {
        self.pocket_representation
    }
    #[getter]
    fn context_representation(&self) -> PyRepresentationHandle {
        self.context_representation
    }
    #[getter]
    fn solvent_representation(&self) -> PyRepresentationHandle {
        self.solvent_representation
    }
}

#[pymethods]
impl PyScene {
    fn focus_with(
        &mut self,
        selection: PySelectionHandle,
        style: PyFocusStyle,
    ) -> PyResult<PyFocusView> {
        core(pdviewx::FocusScene::focus_with(
            &mut self.inner,
            selection.0,
            style.0,
        ))
        .map(Into::into)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyFocusBand>()?;
    module.add_class::<PyFocusContext>()?;
    module.add_class::<PyFocusSurfaceExtent>()?;
    module.add_class::<PyDistanceBands>()?;
    module.add_class::<PyFocusStyle>()?;
    module.add_class::<PyFocusView>()
}
