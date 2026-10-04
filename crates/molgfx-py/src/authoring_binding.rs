//! Immutable color authoring values and namespace.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;

/// Three coordinates.
type Triple = (f32, f32, f32);

#[derive(Clone, Debug)]
#[pyclass(name = "ColorSpec", frozen, skip_from_py_object)]
pub(super) struct PyColorSpec(pub(super) molgfx::ColorSpec);

#[derive(Clone, Debug)]
#[pyclass(name = "RenderProfile", frozen, skip_from_py_object)]
pub(super) struct PyRenderProfile(pub(super) molgfx::RenderProfile);

#[derive(Clone, Debug)]
#[pyclass(name = "Camera", frozen, skip_from_py_object)]
pub(super) struct PyCamera(pub(super) molgfx::Camera);

#[derive(Clone, Debug)]
#[pyclass(name = "ScalarProperty", frozen, skip_from_py_object)]
pub(super) struct PyScalarProperty(pub(super) molgfx::ScalarProperty);

#[pymethods]
impl PyCamera {
    #[new]
    #[pyo3(signature = (*, position, target, up=(0.0, 1.0, 0.0), fov_y=None, aspect=1.0, near=0.1, far=10_000.0))]
    fn new(
        position: (f32, f32, f32),
        target: (f32, f32, f32),
        up: (f32, f32, f32),
        fov_y: Option<f32>,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> PyResult<Self> {
        let mut fov_y_radians = std::f32::consts::FRAC_PI_4;
        if let Some(value) = fov_y {
            fov_y_radians = value;
        }
        molgfx::camera::perspective(
            [position.0, position.1, position.2],
            [target.0, target.1, target.2],
            [up.0, up.1, up.2],
            fov_y_radians,
            aspect,
            near,
            far,
        )
        .map(Self)
        .map_err(crate::binding::error)
    }

    #[getter]
    fn position(&self) -> (f32, f32, f32) {
        (self.0.eye.x, self.0.eye.y, self.0.eye.z)
    }

    #[getter]
    fn target(&self) -> (f32, f32, f32) {
        (self.0.target.x, self.0.target.y, self.0.target.z)
    }

    #[getter]
    fn up(&self) -> (f32, f32, f32) {
        (self.0.up.x, self.0.up.y, self.0.up.z)
    }

    /// Where a world point lands on a render target of `size`, as pixels from
    /// the top-left plus the distance from the eye, or `None` behind the eye.
    fn project(&self, point: (f32, f32, f32), size: (u32, u32)) -> Option<(f32, f32, f32)> {
        molgfx::camera::project(&self.0, [point.0, point.1, point.2], size)
            .map(|screen| (screen.x, screen.y, screen.depth))
    }

    /// The ray through a pixel of a render target of `size`, as an origin and a
    /// unit direction, or `None` for a degenerate camera.
    fn ray(&self, x: f32, y: f32, size: (u32, u32)) -> Option<(Triple, Triple)> {
        molgfx::camera::ray(&self.0, x, y, size).map(|ray| {
            (
                (ray.origin.x, ray.origin.y, ray.origin.z),
                (ray.direction.x, ray.direction.y, ray.direction.z),
            )
        })
    }
}

#[pymethods]
impl PyRenderProfile {
    #[getter]
    const fn target_fps(&self) -> u16 {
        self.0.target_fps
    }

    #[getter]
    const fn quality(&self) -> &'static str {
        match self.0.quality {
            molgfx::Quality::Auto => "auto",
            molgfx::Quality::Interactive => "interactive",
            molgfx::Quality::HighestFixed => "highest_fixed",
            molgfx::Quality::Converged => "converged",
        }
    }

    fn with_effect(&self, effect: &crate::effect_binding::PyEffect) -> PyResult<Self> {
        self.0
            .with_effect(effect.0)
            .map(Self)
            .map_err(crate::binding::error)
    }

    fn without_effect(&self, effect: &crate::effect_binding::PyEffect) -> Self {
        Self(self.0.without_effect(effect.0.kind()))
    }

    fn effect(
        &self,
        effect: &crate::effect_binding::PyEffect,
    ) -> Option<crate::effect_binding::PyEffect> {
        self.0
            .effect(effect.0.kind())
            .map(crate::effect_binding::PyEffect)
    }
}

#[pymethods]
impl PyColorSpec {
    fn legend_json(&self) -> PyResult<Option<String>> {
        self.0
            .legend()
            .map(|legend| {
                serde_json::to_string(&legend)
                    .map_err(|error| PyValueError::new_err(error.to_string()))
            })
            .transpose()
    }
}

#[pyfunction]
fn element() -> PyColorSpec {
    PyColorSpec(molgfx::color::element())
}

#[pyfunction]
fn chain() -> PyColorSpec {
    PyColorSpec(molgfx::color::chain())
}

#[pyfunction]
fn residue() -> PyColorSpec {
    PyColorSpec(molgfx::color::residue())
}

#[pyfunction]
fn secondary_structure() -> PyColorSpec {
    PyColorSpec(molgfx::color::secondary_structure())
}

#[pyfunction]
fn entity() -> PyColorSpec {
    PyColorSpec(molgfx::color::entity())
}

#[pyfunction]
fn molecule_type() -> PyColorSpec {
    PyColorSpec(molgfx::color::molecule_type())
}

#[pyfunction]
fn residue_name() -> PyColorSpec {
    PyColorSpec(molgfx::color::residue_name())
}

#[pyfunction]
fn carbon_by_chain() -> PyColorSpec {
    PyColorSpec(molgfx::color::carbon_by_chain())
}

/// A categorical colour restricted to a named palette.
#[pyfunction]
#[pyo3(signature = (by, *, palette=None, carbon_only=false))]
fn category(by: &str, palette: Option<&str>, carbon_only: bool) -> PyResult<PyColorSpec> {
    let category = molgfx::color::AtomCategory::from_name(by)
        .ok_or_else(|| PyValueError::new_err(format!("unknown category '{by}'")))?;
    let mut spec = molgfx::ColorSpec::category(category);
    if let Some(palette) = palette {
        spec = spec.with_palette(palette);
    }
    if carbon_only {
        spec = spec.carbon_only();
    }
    Ok(PyColorSpec(spec))
}

/// A colour by a value the structure defines for itself.
#[pyfunction]
#[pyo3(signature = (name, *, ramp=None, domain=None))]
fn metric(name: &str, ramp: Option<&str>, domain: Option<(f32, f32)>) -> PyResult<PyColorSpec> {
    let metric = molgfx::color::AtomMetric::from_name(name)
        .ok_or_else(|| PyValueError::new_err(format!("unknown metric '{name}'")))?;
    let mut spec = molgfx::color::metric(metric);
    if let Some(ramp) = ramp {
        spec = spec.with_ramp(ramp);
    }
    if let Some((low, high)) = domain {
        spec = spec.with_domain([low, high]);
    }
    Ok(PyColorSpec(spec))
}

#[pyfunction]
fn ramp_names() -> Vec<&'static str> {
    molgfx::color::ramp_names()
}

#[pyfunction]
fn palette_names() -> Vec<&'static str> {
    molgfx::color::palette_names()
}

#[pyfunction]
fn uniform(rgb: (u8, u8, u8)) -> PyColorSpec {
    PyColorSpec(molgfx::color::uniform(molgfx::Color::rgb(
        rgb.0, rgb.1, rgb.2,
    )))
}

#[pyfunction]
#[pyo3(signature = (*, property, ramp, domain, units=None, missing=(128, 128, 128)))]
fn property(
    property: &PyScalarProperty,
    ramp: &str,
    domain: (f32, f32),
    units: Option<&str>,
    missing: (u8, u8, u8),
) -> PyColorSpec {
    PyColorSpec(molgfx::color::property(
        property.0.clone(),
        ramp,
        [domain.0, domain.1],
        units.map(Into::into),
        molgfx::Color::rgb(missing.0, missing.1, missing.2),
    ))
}

#[pyfunction]
fn interactive() -> PyRenderProfile {
    PyRenderProfile(molgfx::profile::interactive())
}

#[pyfunction]
fn converged() -> PyRenderProfile {
    PyRenderProfile(molgfx::profile::converged())
}

#[pyfunction]
fn adaptive(target_fps: u16) -> PyRenderProfile {
    PyRenderProfile(molgfx::profile::adaptive(target_fps))
}

#[pyfunction]
fn highest_fixed(target_fps: u16) -> PyRenderProfile {
    PyRenderProfile(molgfx::profile::highest_fixed(target_fps))
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyColorSpec>()?;
    module.add_class::<PyRenderProfile>()?;
    module.add_class::<PyCamera>()?;
    module.add_class::<PyScalarProperty>()?;
    let color = PyModule::new(module.py(), "color")?;
    color.add_function(wrap_pyfunction!(element, &color)?)?;
    color.add_function(wrap_pyfunction!(chain, &color)?)?;
    color.add_function(wrap_pyfunction!(residue, &color)?)?;
    color.add_function(wrap_pyfunction!(secondary_structure, &color)?)?;
    color.add_function(wrap_pyfunction!(entity, &color)?)?;
    color.add_function(wrap_pyfunction!(molecule_type, &color)?)?;
    color.add_function(wrap_pyfunction!(residue_name, &color)?)?;
    color.add_function(wrap_pyfunction!(carbon_by_chain, &color)?)?;
    color.add_function(wrap_pyfunction!(category, &color)?)?;
    color.add_function(wrap_pyfunction!(metric, &color)?)?;
    color.add_function(wrap_pyfunction!(ramp_names, &color)?)?;
    color.add_function(wrap_pyfunction!(palette_names, &color)?)?;
    color.add_function(wrap_pyfunction!(uniform, &color)?)?;
    color.add_function(wrap_pyfunction!(property, &color)?)?;
    module.add_submodule(&color)?;
    let profile = PyModule::new(module.py(), "profile")?;
    profile.add_function(wrap_pyfunction!(interactive, &profile)?)?;
    profile.add_function(wrap_pyfunction!(converged, &profile)?)?;
    profile.add_function(wrap_pyfunction!(adaptive, &profile)?)?;
    profile.add_function(wrap_pyfunction!(highest_fixed, &profile)?)?;
    module.add_submodule(&profile)
}
