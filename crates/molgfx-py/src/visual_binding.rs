//! Typed immutable visual-expression adapters.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

#[derive(Clone, Debug)]
#[pyclass(name = "ScalarExpr", frozen, skip_from_py_object)]
pub(super) struct PyScalarExpr(pub(super) molgfx::visual::ScalarExpr);

#[derive(Clone, Debug)]
#[pyclass(name = "ScalarParameter", frozen, skip_from_py_object)]
pub(super) struct PyScalarParameter(pub(super) molgfx::visual::Parameter<f32>);

#[derive(Clone, Debug)]
#[pyclass(name = "VectorExpr", frozen, skip_from_py_object)]
pub(super) struct PyVectorExpr(pub(super) molgfx::visual::VectorExpr);

#[derive(Clone, Debug)]
#[pyclass(name = "VectorParameter", frozen, skip_from_py_object)]
pub(super) struct PyVectorParameter(pub(super) molgfx::visual::Parameter<[f32; 3]>);

#[derive(Clone, Debug)]
#[pyclass(name = "BoolExpr", frozen, skip_from_py_object)]
pub(super) struct PyBoolExpr(pub(super) molgfx::visual::BoolExpr);

#[derive(Clone, Debug)]
#[pyclass(name = "ColorExpr", frozen, skip_from_py_object)]
pub(super) struct PyColorExpr(pub(super) molgfx::visual::ColorExpr);

#[derive(Clone, Debug)]
#[pyclass(name = "ColorParameter", frozen, skip_from_py_object)]
pub(super) struct PyColorParameter(pub(super) molgfx::visual::Parameter<molgfx::Color>);

#[derive(Clone, Debug)]
#[pyclass(name = "VisualStyle", frozen, skip_from_py_object)]
pub(super) struct PyVisualStyle(pub(super) molgfx::VisualStyle);

#[pymethods]
impl PyScalarExpr {
    fn __add__(&self, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self(self.0.clone() + scalar_expression(right)?))
    }

    fn __mul__(&self, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self(self.0.clone() * scalar_expression(right)?))
    }

    fn clamp(&self, minimum: f32, maximum: f32) -> Self {
        Self(self.0.clone().clamp(minimum, maximum))
    }

    fn less(&self, right: &Bound<'_, PyAny>) -> PyResult<PyBoolExpr> {
        Ok(PyBoolExpr(self.0.clone().less(scalar_expression(right)?)))
    }
}

#[pymethods]
impl PyScalarParameter {
    #[getter]
    fn name(&self) -> &str {
        self.0.name()
    }

    #[getter]
    fn default(&self) -> f32 {
        *self.0.default_value()
    }

    fn expression(&self) -> PyScalarExpr {
        PyScalarExpr(self.0.clone().into())
    }
}

#[pymethods]
impl PyVectorExpr {
    fn __add__(&self, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self(self.0.clone() + vector_expression(right)?))
    }

    fn __mul__(&self, right: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self(self.0.clone() * scalar_expression(right)?))
    }

    fn normalized(&self) -> Self {
        Self(self.0.clone().normalized())
    }

    fn dot(&self, right: &Bound<'_, PyAny>) -> PyResult<PyScalarExpr> {
        Ok(PyScalarExpr(self.0.clone().dot(vector_expression(right)?)))
    }
}

#[pymethods]
impl PyVectorParameter {
    #[getter]
    fn name(&self) -> &str {
        self.0.name()
    }

    #[getter]
    fn default(&self) -> (f32, f32, f32) {
        let value = self.0.default_value();
        (value[0], value[1], value[2])
    }

    fn expression(&self) -> PyVectorExpr {
        PyVectorExpr(self.0.clone().into())
    }
}

#[pymethods]
impl PyBoolExpr {
    fn __and__(&self, right: &Self) -> Self {
        Self(self.0.clone() & right.0.clone())
    }

    fn __or__(&self, right: &Self) -> Self {
        Self(self.0.clone() | right.0.clone())
    }

    fn __invert__(&self) -> Self {
        Self(!self.0.clone())
    }
}

#[pymethods]
impl PyColorParameter {
    #[getter]
    fn name(&self) -> &str {
        self.0.name()
    }

    #[getter]
    fn default(&self) -> (u8, u8, u8) {
        let value = self.0.default_value().0;
        (value[0], value[1], value[2])
    }

    fn expression(&self) -> PyColorExpr {
        PyColorExpr(self.0.clone().into())
    }
}

#[pymethods]
impl PyVisualStyle {
    fn stable_hash(&self) -> String {
        self.0.stable_hash()
    }

    fn explain(&self) -> String {
        self.0.explain()
    }

    fn wgsl(&self) -> String {
        self.0.wgsl()
    }
}

fn scalar_expression(value: &Bound<'_, PyAny>) -> PyResult<molgfx::visual::ScalarExpr> {
    if let Ok(expression) = value.extract::<PyRef<'_, PyScalarExpr>>() {
        return Ok(expression.0.clone());
    }
    if let Ok(parameter) = value.extract::<PyRef<'_, PyScalarParameter>>() {
        return Ok(parameter.0.clone().into());
    }
    value.extract::<f32>().map(Into::into).map_err(|_| {
        PyValueError::new_err("expected a scalar expression, parameter, or finite number")
    })
}

fn bool_expression(value: &Bound<'_, PyAny>) -> PyResult<molgfx::visual::BoolExpr> {
    if let Ok(expression) = value.extract::<PyRef<'_, PyBoolExpr>>() {
        return Ok(expression.0.clone());
    }
    value
        .extract::<bool>()
        .map(molgfx::visual::BoolExpr::Constant)
        .map_err(|_| PyValueError::new_err("expected a boolean expression or bool"))
}

fn vector_expression(value: &Bound<'_, PyAny>) -> PyResult<molgfx::visual::VectorExpr> {
    if let Ok(expression) = value.extract::<PyRef<'_, PyVectorExpr>>() {
        return Ok(expression.0.clone());
    }
    if let Ok(parameter) = value.extract::<PyRef<'_, PyVectorParameter>>() {
        return Ok(parameter.0.clone().into());
    }
    value
        .extract::<(f32, f32, f32)>()
        .map(|value| [value.0, value.1, value.2].into())
        .map_err(|_| PyValueError::new_err("expected a vector expression, parameter, or tuple"))
}

fn color_expression(value: &Bound<'_, PyAny>) -> PyResult<molgfx::visual::ColorExpr> {
    if let Ok(expression) = value.extract::<PyRef<'_, PyColorExpr>>() {
        return Ok(expression.0.clone());
    }
    if let Ok(parameter) = value.extract::<PyRef<'_, PyColorParameter>>() {
        return Ok(parameter.0.clone().into());
    }
    value
        .extract::<(u8, u8, u8)>()
        .map(|(red, green, blue)| molgfx::Color::rgb(red, green, blue).into())
        .map_err(|_| PyValueError::new_err("expected a color expression or RGB tuple"))
}

#[pyfunction]
fn scalar(value: f32) -> PyScalarExpr {
    PyScalarExpr(value.into())
}

#[pyfunction]
fn property(name: &str) -> PyScalarExpr {
    PyScalarExpr(molgfx::visual::ScalarExpr::property(name))
}

#[pyfunction]
#[pyo3(signature = (default, *, name))]
fn parameter(default: f32, name: &str) -> PyScalarParameter {
    PyScalarParameter(molgfx::visual::Parameter::new(name, default))
}

#[pyfunction]
fn vector(value: (f32, f32, f32)) -> PyVectorExpr {
    PyVectorExpr([value.0, value.1, value.2].into())
}

#[pyfunction]
fn vector_property(name: &str) -> PyVectorExpr {
    PyVectorExpr(molgfx::visual::VectorExpr::property(name))
}

#[pyfunction]
#[pyo3(signature = (default, *, name))]
fn vector_parameter(default: (f32, f32, f32), name: &str) -> PyVectorParameter {
    PyVectorParameter(molgfx::visual::Parameter::new(
        name,
        [default.0, default.1, default.2],
    ))
}

#[pyfunction]
#[pyo3(signature = (default, *, name))]
fn color_parameter(default: (u8, u8, u8), name: &str) -> PyColorParameter {
    PyColorParameter(molgfx::visual::Parameter::new(
        name,
        molgfx::Color::rgb(default.0, default.1, default.2),
    ))
}

#[pyfunction]
fn state(name: &str) -> PyBoolExpr {
    PyBoolExpr(molgfx::visual::BoolExpr::state(name))
}

#[pyfunction(name = "color")]
fn color_value(rgb: (u8, u8, u8)) -> PyColorExpr {
    PyColorExpr(molgfx::Color::rgb(rgb.0, rgb.1, rgb.2).into())
}

#[pyfunction]
#[pyo3(signature = (value, *, palette, domain, missing=(128, 128, 128)))]
fn ramp(
    value: &Bound<'_, PyAny>,
    palette: &str,
    domain: (f32, f32),
    missing: (u8, u8, u8),
) -> PyResult<PyColorExpr> {
    Ok(PyColorExpr(molgfx::visual::ColorExpr::Ramp {
        value: scalar_expression(value)?,
        palette: palette.into(),
        domain: [domain.0, domain.1],
        missing: molgfx::Color::rgb(missing.0, missing.1, missing.2),
    }))
}

#[pyfunction(name = "where")]
fn where_color(
    condition: &PyBoolExpr,
    yes: &Bound<'_, PyAny>,
    no: &Bound<'_, PyAny>,
) -> PyResult<PyColorExpr> {
    Ok(PyColorExpr(molgfx::visual::ColorExpr::Select {
        condition: condition.0.clone(),
        yes: Box::new(color_expression(yes)?),
        no: Box::new(color_expression(no)?),
    }))
}

#[pyfunction]
#[pyo3(signature = (*, color, opacity=None, visible=None))]
fn style(
    color: &Bound<'_, PyAny>,
    opacity: Option<&Bound<'_, PyAny>>,
    visible: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyVisualStyle> {
    let opacity = match opacity {
        Some(value) => scalar_expression(value)?,
        None => 1.0.into(),
    };
    let visible = match visible {
        Some(value) => bool_expression(value)?,
        None => molgfx::visual::BoolExpr::Constant(true),
    };
    let style = molgfx::VisualStyle::new(color_expression(color)?, opacity, visible);
    let _ = style.compile().map_err(crate::binding::error)?;
    Ok(PyVisualStyle(style))
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyScalarExpr>()?;
    module.add_class::<PyScalarParameter>()?;
    module.add_class::<PyVectorExpr>()?;
    module.add_class::<PyVectorParameter>()?;
    module.add_class::<PyBoolExpr>()?;
    module.add_class::<PyColorExpr>()?;
    module.add_class::<PyColorParameter>()?;
    module.add_class::<PyVisualStyle>()?;
    let visual = PyModule::new(module.py(), "visual")?;
    visual.add_function(wrap_pyfunction!(scalar, &visual)?)?;
    visual.add_function(wrap_pyfunction!(property, &visual)?)?;
    visual.add_function(wrap_pyfunction!(parameter, &visual)?)?;
    visual.add_function(wrap_pyfunction!(vector, &visual)?)?;
    visual.add_function(wrap_pyfunction!(vector_property, &visual)?)?;
    visual.add_function(wrap_pyfunction!(vector_parameter, &visual)?)?;
    visual.add_function(wrap_pyfunction!(color_parameter, &visual)?)?;
    visual.add_function(wrap_pyfunction!(state, &visual)?)?;
    visual.add_function(wrap_pyfunction!(color_value, &visual)?)?;
    visual.add_function(wrap_pyfunction!(ramp, &visual)?)?;
    visual.add_function(wrap_pyfunction!(where_color, &visual)?)?;
    visual.add_function(wrap_pyfunction!(style, &visual)?)?;
    module.add_submodule(&visual)
}
