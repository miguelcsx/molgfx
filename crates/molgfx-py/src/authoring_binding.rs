//! Immutable color authoring values and namespace.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;

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
            molgfx::Quality::Publication => "publication",
        }
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
fn publication() -> PyRenderProfile {
    PyRenderProfile(molgfx::profile::publication())
}

#[pyfunction]
fn adaptive(target_fps: u16) -> PyRenderProfile {
    PyRenderProfile(molgfx::profile::adaptive(target_fps))
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
    color.add_function(wrap_pyfunction!(uniform, &color)?)?;
    color.add_function(wrap_pyfunction!(property, &color)?)?;
    module.add_submodule(&color)?;
    let profile = PyModule::new(module.py(), "profile")?;
    profile.add_function(wrap_pyfunction!(interactive, &profile)?)?;
    profile.add_function(wrap_pyfunction!(publication, &profile)?)?;
    profile.add_function(wrap_pyfunction!(adaptive, &profile)?)?;
    module.add_submodule(&profile)
}
