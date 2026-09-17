//! Thin Python adapters for Rust-validated visual programs.

#[path = "visual_compatibility.rs"]
mod compatibility;
#[path = "visual_enums.rs"]
mod enums;
#[path = "visual_expressions.rs"]
mod expressions;
#[path = "visual_inputs.rs"]
mod inputs;
#[path = "visual_register.rs"]
mod register;
pub(crate) use compatibility::PyVisualCompatibility;
pub(crate) use enums::{PyVisualOutput, PyVisualStage};
pub(crate) use expressions::{
    PyBoolExpr, PyColorExpr, PyColorParameter, PyScalarExpr, PyScalarParameter, PyVectorExpr,
    PyVectorParameter,
};
pub(crate) use register::register;

use crate::core::PyAttributeHandle;
use crate::error::{value, visual};
use crate::math::PyRgba8;
use crate::values::PyScalarRamp;
use pyo3::prelude::*;

#[pyclass(name = "VisualInputs", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVisualInputs(molgfx::VisualInputs);

#[pymethods]
impl PyVisualInputs {
    #[new]
    #[pyo3(signature = (
        base_color=(1.0, 1.0, 1.0, 1.0), base_opacity=1.0, time_seconds=0.0,
        local_position=(0.0, 0.0, 0.0), world_position=(0.0, 0.0, 0.0),
        normal=(0.0, 0.0, 1.0), view_direction=(0.0, 0.0, 1.0),
        camera_distance=0.0, entity_index=0, roughness=0.34, specular=0.5,
        material_strength=0.0, properties=None
    ))]
    fn new(
        base_color: (f32, f32, f32, f32),
        base_opacity: f32,
        time_seconds: f32,
        local_position: (f32, f32, f32),
        world_position: (f32, f32, f32),
        normal: (f32, f32, f32),
        view_direction: (f32, f32, f32),
        camera_distance: f32,
        entity_index: u32,
        roughness: f32,
        specular: f32,
        material_strength: f32,
        properties: Option<(f32, f32, f32, f32)>,
    ) -> Self {
        let properties = properties
            .map_or((f32::NAN, f32::NAN, f32::NAN, f32::NAN), |properties| {
                properties
            });
        Self(molgfx::VisualInputs {
            base_color: [base_color.0, base_color.1, base_color.2, base_color.3],
            base_opacity,
            time_seconds,
            local_position: [local_position.0, local_position.1, local_position.2],
            world_position: [world_position.0, world_position.1, world_position.2],
            normal: [normal.0, normal.1, normal.2],
            view_direction: [view_direction.0, view_direction.1, view_direction.2],
            camera_distance,
            entity_index,
            roughness,
            specular,
            material_strength,
            attributes: [[f32::NAN; 4]; 4],
            properties: [properties.0, properties.1, properties.2, properties.3],
        })
    }
}

#[pyclass(name = "VisualEvaluation", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVisualEvaluation(molgfx::VisualEvaluation);

#[pymethods]
impl PyVisualEvaluation {
    #[getter]
    fn base_color(&self) -> (f32, f32, f32, f32) {
        self.0.base_color.into()
    }
    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }
    #[getter]
    fn emission(&self) -> (f32, f32, f32) {
        self.0.emission.into()
    }
    #[getter]
    fn roughness(&self) -> f32 {
        self.0.roughness
    }
    #[getter]
    fn specular(&self) -> f32 {
        self.0.specular
    }
    #[getter]
    fn material_strength(&self) -> f32 {
        self.0.material_strength
    }
    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }
    #[getter]
    fn silhouette_softness(&self) -> f32 {
        self.0.silhouette_softness
    }
    #[getter]
    fn radius_scale(&self) -> f32 {
        self.0.radius_scale
    }
    #[getter]
    fn width_scale(&self) -> f32 {
        self.0.width_scale
    }
    #[getter]
    fn position_offset(&self) -> (f32, f32, f32) {
        self.0.position_offset.into()
    }
}

#[pyclass(name = "VisualProgram", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyVisualProgram(pub(crate) molgfx::VisualProgram);

#[pymethods]
impl PyVisualProgram {
    #[getter]
    fn fingerprint(&self) -> u64 {
        self.0.fingerprint()
    }
    #[getter]
    fn instruction_count(&self) -> usize {
        self.0.instructions().len()
    }
    #[getter]
    fn uniform_instruction_count(&self) -> usize {
        self.0.uniform_instruction_count()
    }
    #[getter]
    fn entity_instruction_count(&self) -> usize {
        self.0.entity_instruction_count()
    }
    #[getter]
    fn fragment_instruction_count(&self) -> usize {
        self.0.fragment_instruction_count()
    }
    #[getter]
    fn maximum_displacement(&self) -> f32 {
        self.0.maximum_displacement()
    }
    fn scalar_parameter(&self, index: usize) -> Option<PyScalarParameter> {
        self.0.scalar_parameter(index).map(PyScalarParameter)
    }
    fn color_parameter(&self, index: usize) -> Option<PyColorParameter> {
        self.0.color_parameter(index).map(PyColorParameter)
    }
    fn vector_parameter(&self, index: usize) -> Option<PyVectorParameter> {
        self.0.vector_parameter(index).map(PyVectorParameter)
    }
    fn evaluate(&self, inputs: PyVisualInputs) -> PyVisualEvaluation {
        PyVisualEvaluation(self.0.evaluate(inputs.0))
    }
    fn __repr__(&self) -> String {
        self.0.to_string()
    }
}

#[pyclass(name = "VisualStyle", from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyVisualStyle(pub(crate) molgfx::VisualStyle);

#[pymethods]
impl PyVisualStyle {
    #[new]
    fn new(program: PyVisualProgram) -> Self {
        Self(molgfx::VisualStyle::new(program.0))
    }

    #[staticmethod]
    fn pulse(color: PyRgba8, cycles_per_second: f32, minimum: f32, maximum: f32) -> PyResult<Self> {
        visual(molgfx::VisualStyle::pulse(
            color.0,
            cycles_per_second,
            minimum,
            maximum,
        ))
        .map(Self)
    }

    #[getter]
    fn program(&self) -> PyVisualProgram {
        PyVisualProgram(self.0.program().clone())
    }
    fn set_scalar(&mut self, parameter: PyScalarParameter, value_: f32) -> PyResult<()> {
        visual(self.0.set_scalar(parameter.0, value_))
    }
    fn set_color(
        &mut self,
        parameter: PyColorParameter,
        value_: (f32, f32, f32, f32),
    ) -> PyResult<()> {
        visual(
            self.0
                .set_color(parameter.0, [value_.0, value_.1, value_.2, value_.3]),
        )
    }
    fn set_vector(
        &mut self,
        parameter: PyVectorParameter,
        value_: (f32, f32, f32),
    ) -> PyResult<()> {
        visual(
            self.0
                .set_vector(parameter.0, [value_.0, value_.1, value_.2]),
        )
    }
    fn evaluate(&self, inputs: PyVisualInputs) -> PyVisualEvaluation {
        PyVisualEvaluation(self.0.evaluate(inputs.0))
    }
}

#[pyclass(name = "VisualProgramBuilder")]
#[derive(Debug)]
pub(crate) struct PyVisualProgramBuilder {
    inner: Option<molgfx::VisualProgramBuilder>,
}

impl PyVisualProgramBuilder {
    fn builder(&mut self) -> PyResult<&mut molgfx::VisualProgramBuilder> {
        self.inner
            .as_mut()
            .ok_or_else(|| value("visual-program builder is already finished"))
    }
}

#[pymethods]
impl PyVisualProgramBuilder {
    fn base_roughness(&mut self) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.base_roughness()).map(PyScalarExpr)
    }
    fn base_specular(&mut self) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.base_specular()).map(PyScalarExpr)
    }
    fn base_material_strength(&mut self) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.base_material_strength()).map(PyScalarExpr)
    }
    fn scalar_attribute(&mut self, attribute: PyAttributeHandle) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.scalar_attribute(attribute.0)).map(PyScalarExpr)
    }
    fn category_attribute(&mut self, attribute: PyAttributeHandle) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.category_attribute(attribute.0)).map(PyScalarExpr)
    }
    fn vector_attribute(&mut self, attribute: PyAttributeHandle) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.vector_attribute(attribute.0)).map(PyVectorExpr)
    }
    fn color_attribute(&mut self, attribute: PyAttributeHandle) -> PyResult<PyColorExpr> {
        visual(self.builder()?.color_attribute(attribute.0)).map(PyColorExpr)
    }
    fn scalar_parameter(&mut self, default: f32) -> PyResult<(PyScalarParameter, PyScalarExpr)> {
        visual(self.builder()?.scalar_parameter(default))
            .map(|(parameter, expression)| (PyScalarParameter(parameter), PyScalarExpr(expression)))
    }
    fn color_parameter(
        &mut self,
        default: (f32, f32, f32, f32),
    ) -> PyResult<(PyColorParameter, PyColorExpr)> {
        visual(
            self.builder()?
                .color_parameter([default.0, default.1, default.2, default.3]),
        )
        .map(|(parameter, expression)| (PyColorParameter(parameter), PyColorExpr(expression)))
    }
    fn vector_parameter(
        &mut self,
        default: (f32, f32, f32),
    ) -> PyResult<(PyVectorParameter, PyVectorExpr)> {
        visual(
            self.builder()?
                .vector_parameter([default.0, default.1, default.2]),
        )
        .map(|(parameter, expression)| (PyVectorParameter(parameter), PyVectorExpr(expression)))
    }

    fn add(&mut self, a: PyScalarExpr, b: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.add(a.0, b.0)).map(PyScalarExpr)
    }
    fn subtract(&mut self, a: PyScalarExpr, b: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.subtract(a.0, b.0)).map(PyScalarExpr)
    }
    fn multiply(&mut self, a: PyScalarExpr, b: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.multiply(a.0, b.0)).map(PyScalarExpr)
    }
    fn safe_divide(&mut self, a: PyScalarExpr, b: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.safe_divide(a.0, b.0)).map(PyScalarExpr)
    }
    fn abs(&mut self, value_: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.abs(value_.0)).map(PyScalarExpr)
    }
    fn minimum(&mut self, a: PyScalarExpr, b: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.minimum(a.0, b.0)).map(PyScalarExpr)
    }
    fn maximum(&mut self, a: PyScalarExpr, b: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.maximum(a.0, b.0)).map(PyScalarExpr)
    }
    fn clamp(
        &mut self,
        value_: PyScalarExpr,
        low: PyScalarExpr,
        high: PyScalarExpr,
    ) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.clamp(value_.0, low.0, high.0)).map(PyScalarExpr)
    }
    fn saturate(&mut self, value_: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.saturate(value_.0)).map(PyScalarExpr)
    }
    fn step(&mut self, edge: PyScalarExpr, value_: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.step(edge.0, value_.0)).map(PyScalarExpr)
    }
    fn smoothstep(
        &mut self,
        low: PyScalarExpr,
        high: PyScalarExpr,
        value_: PyScalarExpr,
    ) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.smoothstep(low.0, high.0, value_.0)).map(PyScalarExpr)
    }
    fn sine(&mut self, value_: PyScalarExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.sine(value_.0)).map(PyScalarExpr)
    }
    fn mix_scalar(
        &mut self,
        a: PyScalarExpr,
        b: PyScalarExpr,
        weight: PyScalarExpr,
    ) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.mix_scalar(a.0, b.0, weight.0)).map(PyScalarExpr)
    }
    fn mix_color(
        &mut self,
        a: PyColorExpr,
        b: PyColorExpr,
        weight: PyScalarExpr,
    ) -> PyResult<PyColorExpr> {
        visual(self.builder()?.mix_color(a.0, b.0, weight.0)).map(PyColorExpr)
    }
    fn ramp(&mut self, value_: PyScalarExpr, ramp: PyScalarRamp) -> PyResult<PyColorExpr> {
        visual(self.builder()?.ramp(value_.0, ramp.0)).map(PyColorExpr)
    }
    fn less(&mut self, a: PyScalarExpr, b: PyScalarExpr) -> PyResult<PyBoolExpr> {
        visual(self.builder()?.less(a.0, b.0)).map(PyBoolExpr)
    }
    fn greater(&mut self, a: PyScalarExpr, b: PyScalarExpr) -> PyResult<PyBoolExpr> {
        visual(self.builder()?.greater(a.0, b.0)).map(PyBoolExpr)
    }
    fn and_(&mut self, a: PyBoolExpr, b: PyBoolExpr) -> PyResult<PyBoolExpr> {
        visual(self.builder()?.and(a.0, b.0)).map(PyBoolExpr)
    }
    fn or_(&mut self, a: PyBoolExpr, b: PyBoolExpr) -> PyResult<PyBoolExpr> {
        visual(self.builder()?.or(a.0, b.0)).map(PyBoolExpr)
    }
    fn not_(&mut self, value_: PyBoolExpr) -> PyResult<PyBoolExpr> {
        visual(self.builder()?.not(value_.0)).map(PyBoolExpr)
    }
    fn select_scalar(
        &mut self,
        condition: PyBoolExpr,
        yes: PyScalarExpr,
        no: PyScalarExpr,
    ) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.select_scalar(condition.0, yes.0, no.0)).map(PyScalarExpr)
    }
    fn select_color(
        &mut self,
        condition: PyBoolExpr,
        yes: PyColorExpr,
        no: PyColorExpr,
    ) -> PyResult<PyColorExpr> {
        visual(self.builder()?.select_color(condition.0, yes.0, no.0)).map(PyColorExpr)
    }
    fn add_vector(&mut self, a: PyVectorExpr, b: PyVectorExpr) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.add_vector(a.0, b.0)).map(PyVectorExpr)
    }
    fn scale_vector(
        &mut self,
        vector: PyVectorExpr,
        scale: PyScalarExpr,
    ) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.scale_vector(vector.0, scale.0)).map(PyVectorExpr)
    }
    fn dot(&mut self, a: PyVectorExpr, b: PyVectorExpr) -> PyResult<PyScalarExpr> {
        visual(self.builder()?.dot(a.0, b.0)).map(PyScalarExpr)
    }
    fn normalize(&mut self, value_: PyVectorExpr) -> PyResult<PyVectorExpr> {
        visual(self.builder()?.normalize(value_.0)).map(PyVectorExpr)
    }

    fn set_base_color(&mut self, value_: PyColorExpr) -> PyResult<()> {
        visual(self.builder()?.set_base_color(value_.0))
    }
    fn set_opacity(&mut self, value_: PyScalarExpr) -> PyResult<()> {
        visual(self.builder()?.set_opacity(value_.0))
    }
    fn set_emission(&mut self, value_: PyColorExpr) -> PyResult<()> {
        visual(self.builder()?.set_emission(value_.0))
    }
    fn set_roughness(&mut self, value_: PyScalarExpr) -> PyResult<()> {
        visual(self.builder()?.set_roughness(value_.0))
    }
    fn set_specular(&mut self, value_: PyScalarExpr) -> PyResult<()> {
        visual(self.builder()?.set_specular(value_.0))
    }
    fn set_material_strength(&mut self, value_: PyScalarExpr) -> PyResult<()> {
        visual(self.builder()?.set_material_strength(value_.0))
    }
    fn set_visibility(&mut self, value_: PyBoolExpr) -> PyResult<()> {
        visual(self.builder()?.set_visibility(value_.0))
    }
    fn set_silhouette_softness(&mut self, value_: PyScalarExpr) -> PyResult<()> {
        visual(self.builder()?.set_silhouette_softness(value_.0))
    }
    fn set_radius_scale(&mut self, value_: PyScalarExpr) -> PyResult<()> {
        visual(self.builder()?.set_radius_scale(value_.0))
    }
    fn set_width_scale(&mut self, value_: PyScalarExpr) -> PyResult<()> {
        visual(self.builder()?.set_width_scale(value_.0))
    }
    fn set_position_offset(
        &mut self,
        value_: PyVectorExpr,
        maximum_displacement: f32,
    ) -> PyResult<()> {
        visual(
            self.builder()?
                .set_position_offset(value_.0, maximum_displacement),
        )
    }

    fn finish(&mut self) -> PyResult<PyVisualProgram> {
        let builder = self
            .inner
            .take()
            .ok_or_else(|| value("visual-program builder is already finished"))?;
        visual(builder.finish()).map(PyVisualProgram)
    }

    fn finish_color(&mut self, value_: PyColorExpr) -> PyResult<PyVisualProgram> {
        let builder = self
            .inner
            .take()
            .ok_or_else(|| value("visual-program builder is already finished"))?;
        visual(builder.finish_color(value_.0)).map(PyVisualProgram)
    }
}
