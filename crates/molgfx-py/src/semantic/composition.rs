//! Generic composition and semantic focus adapters.
//!
//! Both families describe a picture as a function of caller-computed columns —
//! an emphasis, a delta, a weight — so the binding carries values and policies
//! and lets the scene do the lowering. The two traits that carry the methods
//! become classes of static methods over a scene, one per composition kind, the
//! way `SurfaceZoneScene` already works.

use crate::core::{
    PyAttributeHandle, PyRepresentationHandle, PyRowDomain, PyScene, PySelectionHandle,
};
use crate::error::{composition, core, focus};
use crate::math::PyRgba8;
use crate::semantic::generic::PyGenericCompositionView;
use crate::values::{PyScalarRamp, PySurfaceStyle};
use pyo3::prelude::*;

#[pyclass(name = "FocusLayer", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFocusLayer(pub(crate) molgfx::semantic::FocusLayer);

#[pymethods]
impl PyFocusLayer {
    #[new]
    fn new(domain: PyRowDomain, emphasis: PyAttributeHandle) -> Self {
        Self(molgfx::semantic::FocusLayer {
            domain: domain.0,
            emphasis: emphasis.0,
        })
    }

    #[getter]
    fn domain(&self) -> PyRowDomain {
        PyRowDomain(self.0.domain)
    }

    #[getter]
    fn emphasis(&self) -> PyAttributeHandle {
        PyAttributeHandle(self.0.emphasis)
    }
}

#[pyclass(name = "FocusCompositionStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFocusCompositionStyle(pub(crate) molgfx::semantic::FocusCompositionStyle);

#[pymethods]
impl PyFocusCompositionStyle {
    #[new]
    #[pyo3(signature = (context_opacity=0.16, focus_opacity=1.0, order=0))]
    fn new(context_opacity: f32, focus_opacity: f32, order: i32) -> Self {
        Self(molgfx::semantic::FocusCompositionStyle {
            context_opacity,
            focus_opacity,
            order,
        })
    }

    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::semantic::FocusCompositionStyle::default())
    }
}

#[pyclass(name = "DifferenceLayer", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDifferenceLayer(pub(crate) molgfx::semantic::DifferenceLayer);

#[pymethods]
impl PyDifferenceLayer {
    #[new]
    fn new(domain: PyRowDomain, delta: PyAttributeHandle) -> Self {
        Self(molgfx::semantic::DifferenceLayer {
            domain: domain.0,
            delta: delta.0,
        })
    }

    #[getter]
    fn domain(&self) -> PyRowDomain {
        PyRowDomain(self.0.domain)
    }

    #[getter]
    fn delta(&self) -> PyAttributeHandle {
        PyAttributeHandle(self.0.delta)
    }
}

#[pyclass(name = "DifferenceCompositionStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDifferenceCompositionStyle(
    pub(crate) molgfx::semantic::DifferenceCompositionStyle,
);

#[pymethods]
impl PyDifferenceCompositionStyle {
    #[new]
    fn new(
        context_threshold: f32,
        emphasis_threshold: f32,
        context_opacity: f32,
        ramp: PyScalarRamp,
        order: i32,
    ) -> Self {
        Self(molgfx::semantic::DifferenceCompositionStyle {
            context_threshold,
            emphasis_threshold,
            context_opacity,
            ramp: ramp.0,
            order,
        })
    }
}

#[pyclass(name = "EnsembleLayer", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyEnsembleLayer(pub(crate) molgfx::semantic::EnsembleLayer);

#[pymethods]
impl PyEnsembleLayer {
    #[new]
    fn new(domain: PyRowDomain, weight: f32, color: PyRgba8) -> Self {
        Self(molgfx::semantic::EnsembleLayer {
            domain: domain.0,
            weight,
            color: color.0,
        })
    }

    #[getter]
    fn domain(&self) -> PyRowDomain {
        PyRowDomain(self.0.domain)
    }

    #[getter]
    fn weight(&self) -> f32 {
        self.0.weight
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        PyRgba8(self.0.color)
    }
}

#[pyclass(name = "EnsembleCompositionStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyEnsembleCompositionStyle(pub(crate) molgfx::semantic::EnsembleCompositionStyle);

#[pymethods]
impl PyEnsembleCompositionStyle {
    #[new]
    #[pyo3(signature = (dominant_opacity=1.0, alternate_opacity=0.55, minimum_opacity=0.08, order=0))]
    fn new(
        dominant_opacity: f32,
        alternate_opacity: f32,
        minimum_opacity: f32,
        order: i32,
    ) -> Self {
        Self(molgfx::semantic::EnsembleCompositionStyle {
            dominant_opacity,
            alternate_opacity,
            minimum_opacity,
            order,
        })
    }

    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::semantic::EnsembleCompositionStyle::default())
    }
}

/// The three declarative compositions over generic row domains.
#[pyclass(name = "GenericCompositionScene", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PyGenericCompositionScene;

#[pymethods]
impl PyGenericCompositionScene {
    #[staticmethod]
    fn compose_focus(
        mut scene: PyRefMut<'_, PyScene>,
        layer: PyFocusLayer,
        style: PyFocusCompositionStyle,
    ) -> PyResult<PyGenericCompositionView> {
        composition(molgfx::semantic::GenericCompositionScene::compose_focus(
            &mut scene.inner,
            layer.0,
            style.0,
        ))
        .map(PyGenericCompositionView::from)
    }

    #[staticmethod]
    fn compose_difference(
        mut scene: PyRefMut<'_, PyScene>,
        layers: Vec<PyDifferenceLayer>,
        style: PyDifferenceCompositionStyle,
    ) -> PyResult<PyGenericCompositionView> {
        let layers: Vec<molgfx::semantic::DifferenceLayer> =
            layers.into_iter().map(|layer| layer.0).collect();
        composition(
            molgfx::semantic::GenericCompositionScene::compose_difference(
                &mut scene.inner,
                &layers,
                style.0,
            ),
        )
        .map(PyGenericCompositionView::from)
    }

    #[staticmethod]
    fn compose_ensemble(
        mut scene: PyRefMut<'_, PyScene>,
        layers: Vec<PyEnsembleLayer>,
        style: PyEnsembleCompositionStyle,
    ) -> PyResult<PyGenericCompositionView> {
        let layers: Vec<molgfx::semantic::EnsembleLayer> =
            layers.into_iter().map(|layer| layer.0).collect();
        composition(molgfx::semantic::GenericCompositionScene::compose_ensemble(
            &mut scene.inner,
            &layers,
            style.0,
        ))
        .map(PyGenericCompositionView::from)
    }
}

/// Strictly increasing distance thresholds, in Ångström.
#[pyclass(name = "DistanceBands", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDistanceBands(pub(crate) molgfx::semantic::DistanceBands);

#[pymethods]
impl PyDistanceBands {
    /// Builds a band pair.
    ///
    /// # Errors
    ///
    /// The thresholds must be finite, non-negative and strictly increasing.
    #[new]
    fn new(near: f32, mid: f32) -> PyResult<Self> {
        focus(molgfx::semantic::DistanceBands::new(near, mid)).map(Self)
    }

    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::semantic::DistanceBands::default())
    }

    #[getter]
    fn near(&self) -> f32 {
        self.0.near()
    }

    #[getter]
    fn mid(&self) -> f32 {
        self.0.mid()
    }

    /// The band a distance falls in.
    fn classify(&self, distance: f32) -> PyFocusBand {
        PyFocusBand(self.0.classify(distance))
    }
}

/// Which of the three spatial bands a distance belongs to.
#[pyclass(name = "FocusBand", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyFocusBand(pub(crate) molgfx::semantic::FocusBand);

#[pymethods]
impl PyFocusBand {
    #[classattr]
    #[pyo3(name = "Near")]
    fn near() -> Self {
        Self(molgfx::semantic::FocusBand::Near)
    }

    #[classattr]
    #[pyo3(name = "Mid")]
    fn mid() -> Self {
        Self(molgfx::semantic::FocusBand::Mid)
    }

    #[classattr]
    #[pyo3(name = "Far")]
    fn far() -> Self {
        Self(molgfx::semantic::FocusBand::Far)
    }

    fn __repr__(&self) -> String {
        format!("FocusBand.{:?}", self.0)
    }
}

/// How polymer context is drawn around a focused subject.
#[pyclass(name = "FocusContext", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyFocusContext(pub(crate) molgfx::semantic::FocusContext);

#[pymethods]
impl PyFocusContext {
    #[classattr]
    #[pyo3(name = "Cartoon")]
    fn cartoon() -> Self {
        Self(molgfx::semantic::FocusContext::Cartoon)
    }

    #[classattr]
    #[pyo3(name = "Trace")]
    fn trace() -> Self {
        Self(molgfx::semantic::FocusContext::Trace)
    }

    #[classattr]
    #[pyo3(name = "Tube")]
    fn tube() -> Self {
        Self(molgfx::semantic::FocusContext::Tube)
    }

    fn __repr__(&self) -> String {
        format!("FocusContext.{:?}", self.0)
    }
}

/// How far the boundary surface around a focused subject extends.
#[pyclass(name = "FocusSurfaceExtent", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyFocusSurfaceExtent(pub(crate) molgfx::semantic::FocusSurfaceExtent);

#[pymethods]
impl PyFocusSurfaceExtent {
    #[classattr]
    #[pyo3(name = "Pocket")]
    fn pocket() -> Self {
        Self(molgfx::semantic::FocusSurfaceExtent::Pocket)
    }

    #[classattr]
    #[pyo3(name = "Structure")]
    fn structure() -> Self {
        Self(molgfx::semantic::FocusSurfaceExtent::Structure)
    }

    fn __repr__(&self) -> String {
        format!("FocusSurfaceExtent.{:?}", self.0)
    }
}

#[pyclass(name = "FocusStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFocusStyle(pub(crate) molgfx::semantic::FocusStyle);

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
        let default = molgfx::semantic::FocusStyle::default();
        Self(molgfx::semantic::FocusStyle {
            bands: bands.map_or(default.bands, |bands| bands.0),
            pocket_opacity,
            context_opacity,
            pocket_presentation: pocket_presentation
                .map_or(default.pocket_presentation, Into::into),
            surface_extent: surface_extent.map_or(default.surface_extent, |extent| extent.0),
            pocket_color: pocket_color.map_or(default.pocket_color, |color| color.0),
            context_geometry: context_geometry.map_or(default.context_geometry, |value| value.0),
            context_color: context_color.map_or(default.context_color, |color| color.0),
            solvent_opacity,
            solvent_color: solvent_color.map_or(default.solvent_color, |color| color.0),
        })
    }

    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::semantic::FocusStyle::default())
    }
}

/// The selections and representations a focus composition produced.
#[pyclass(name = "FocusView", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFocusView(pub(crate) molgfx::semantic::FocusView);

#[pymethods]
impl PyFocusView {
    #[getter]
    fn focus(&self) -> PySelectionHandle {
        self.0.focus.into()
    }

    #[getter]
    fn near(&self) -> PySelectionHandle {
        self.0.near.into()
    }

    #[getter]
    fn pocket(&self) -> PySelectionHandle {
        self.0.pocket.into()
    }

    #[getter]
    fn mid(&self) -> PySelectionHandle {
        self.0.mid.into()
    }

    #[getter]
    fn context(&self) -> PySelectionHandle {
        self.0.context.into()
    }

    #[getter]
    fn solvent(&self) -> PySelectionHandle {
        self.0.solvent.into()
    }

    #[getter]
    fn focus_representation(&self) -> PyRepresentationHandle {
        self.0.focus_representation.into()
    }

    #[getter]
    fn near_representation(&self) -> PyRepresentationHandle {
        self.0.near_representation.into()
    }

    #[getter]
    fn mid_representation(&self) -> PyRepresentationHandle {
        self.0.mid_representation.into()
    }

    #[getter]
    fn pocket_representation(&self) -> PyRepresentationHandle {
        self.0.pocket_representation.into()
    }

    #[getter]
    fn context_representation(&self) -> PyRepresentationHandle {
        self.0.context_representation.into()
    }
}

/// The focus-and-context composition over a molecular scene.
#[pyclass(name = "FocusScene", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PyFocusScene;

#[pymethods]
impl PyFocusScene {
    #[staticmethod]
    fn focus(
        mut scene: PyRefMut<'_, PyScene>,
        selection: PySelectionHandle,
    ) -> PyResult<PyFocusView> {
        core(molgfx::semantic::FocusScene::focus(
            &mut scene.inner,
            selection.0,
        ))
        .map(PyFocusView)
    }

    #[staticmethod]
    #[pyo3(signature = (scene, selection, style=None))]
    fn focus_with(
        mut scene: PyRefMut<'_, PyScene>,
        selection: PySelectionHandle,
        style: Option<PyFocusStyle>,
    ) -> PyResult<PyFocusView> {
        let style = style.map_or_else(molgfx::semantic::FocusStyle::default, |style| style.0);
        core(molgfx::semantic::FocusScene::focus_with(
            &mut scene.inner,
            selection.0,
            style,
        ))
        .map(PyFocusView)
    }
}
