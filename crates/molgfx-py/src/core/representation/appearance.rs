//! Property-driven opacity and edge-softness appearance adapters.

use crate::core::PyAtomPropertyHandle;
use crate::error::core;
use pyo3::prelude::*;

/// Resolved visual response for one property value.
#[pyclass(name = "PropertyAppearanceSample", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyPropertyAppearanceSample(pub(crate) molgfx::core::PropertyAppearanceSample);

#[pymethods]
impl PyPropertyAppearanceSample {
    #[new]
    fn new(opacity: f32, softness_pixels: f32) -> Self {
        Self(molgfx::core::PropertyAppearanceSample {
            opacity,
            softness_pixels,
        })
    }

    /// Opacity multiplier in `[0, 1]`.
    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }

    /// Analytic silhouette transition width in physical pixels.
    #[getter]
    fn softness_pixels(&self) -> f32 {
        self.0.softness_pixels
    }

    fn __repr__(&self) -> String {
        format!(
            "PropertyAppearanceSample(opacity={}, softness_pixels={})",
            self.0.opacity, self.0.softness_pixels
        )
    }
}

/// Reversible scalar-to-opacity-and-edge-softness encoding.
///
/// This is an uncertainty presentation, not a spatial error model: it never
/// changes an atom radius or claims that softness is measured in Ångström.
#[pyclass(name = "PropertyAppearance", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPropertyAppearance(pub(crate) molgfx::core::PropertyAppearance);

impl PyPropertyAppearance {
    /// Endpoint and missing responses, in the order the serializer writes them.
    fn responses(&self) -> ([f32; 2], [f32; 2], [f32; 2], [f32; 2]) {
        self.0.description_values()
    }
}

#[pymethods]
impl PyPropertyAppearance {
    #[new]
    fn new(
        property: PyAtomPropertyHandle,
        domain: (f32, f32),
        opacity: (f32, f32),
        softness_pixels: (f32, f32),
        missing: PyPropertyAppearanceSample,
    ) -> PyResult<Self> {
        core(molgfx::core::PropertyAppearance::new(
            property.0,
            [domain.0, domain.1],
            [opacity.0, opacity.1],
            [softness_pixels.0, softness_pixels.1],
            missing.0,
        ))
        .map(Self)
    }

    /// Honest confidence-rank encoding: low values are diffuse and faint,
    /// high values are crisp and opaque.
    #[staticmethod]
    fn confidence(property: PyAtomPropertyHandle, domain: (f32, f32)) -> PyResult<Self> {
        core(molgfx::core::PropertyAppearance::confidence(
            property.0,
            [domain.0, domain.1],
        ))
        .map(Self)
    }

    /// Flexibility-rank encoding: rigid values remain crisp while mobile
    /// values become diffuse and less opaque.
    #[staticmethod]
    fn flexibility(property: PyAtomPropertyHandle, domain: (f32, f32)) -> PyResult<Self> {
        core(molgfx::core::PropertyAppearance::flexibility(
            property.0,
            [domain.0, domain.1],
        ))
        .map(Self)
    }

    /// Column sampled in source atom-row order.
    #[getter]
    fn property(&self) -> PyAtomPropertyHandle {
        PyAtomPropertyHandle(self.0.property)
    }

    /// Scientific value interval both channels represent.
    #[getter]
    fn domain(&self) -> (f32, f32) {
        let domain = self.0.domain();
        (domain[0], domain[1])
    }

    /// Opacity at the domain endpoints.
    #[getter]
    fn opacity(&self) -> (f32, f32) {
        let (_, opacity, _, _) = self.responses();
        (opacity[0], opacity[1])
    }

    /// Silhouette softness at the domain endpoints.
    #[getter]
    fn softness_pixels(&self) -> (f32, f32) {
        let (_, _, softness, _) = self.responses();
        (softness[0], softness[1])
    }

    /// Response used for absent and non-finite values.
    #[getter]
    fn missing(&self) -> PyPropertyAppearanceSample {
        let (_, _, _, missing) = self.responses();
        PyPropertyAppearanceSample(molgfx::core::PropertyAppearanceSample {
            opacity: missing[0],
            softness_pixels: missing[1],
        })
    }

    /// True when any finite or missing sample can require alpha composition.
    #[getter]
    fn is_translucent(&self) -> bool {
        self.0.is_translucent()
    }

    /// Resolves a finite value, clamped to the declared domain; a NaN uses the
    /// explicit missing response.
    fn sample(&self, value_: f32) -> PyPropertyAppearanceSample {
        PyPropertyAppearanceSample(self.0.sample(value_))
    }

    /// Recovers the represented value from opacity when that channel varies.
    fn value_from_opacity(&self, opacity: f32) -> Option<f32> {
        self.0.value_from_opacity(opacity)
    }

    /// Recovers the represented value from softness when that channel varies.
    fn value_from_softness(&self, softness_pixels: f32) -> Option<f32> {
        self.0.value_from_softness(softness_pixels)
    }

    fn __repr__(&self) -> String {
        let (domain, opacity, softness, missing) = self.responses();
        let property = self.0.property;
        format!(
            "PropertyAppearance(property=AtomPropertyHandle({}, {}), domain=({}, {}), \
             opacity=({}, {}), softness_pixels=({}, {}), \
             missing=PropertyAppearanceSample(opacity={}, softness_pixels={}))",
            property.row(),
            property.generation(),
            domain[0],
            domain[1],
            opacity[0],
            opacity[1],
            softness[0],
            softness[1],
            missing[0],
            missing[1]
        )
    }
}
