//! Curve sampling and the small numeric packs that feed a packed column.

use crate::error::value;
use crate::math::PyVec3;
use pyo3::prelude::*;

#[pyclass(name = "Vec2", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyVec2(pub(crate) molgfx::math::Vec2);

#[pymethods]
impl PyVec2 {
    #[new]
    fn new(x: f32, y: f32) -> Self {
        Self(molgfx::math::Vec2::new(x, y))
    }

    #[classattr]
    #[pyo3(name = "ZERO")]
    fn zero() -> Self {
        Self(molgfx::math::Vec2::ZERO)
    }

    #[getter]
    fn x(&self) -> f32 {
        self.0.x
    }

    #[getter]
    fn y(&self) -> f32 {
        self.0.y
    }

    #[pyo3(name = "to_tuple")]
    fn tuple(&self) -> (f32, f32) {
        (self.0.x, self.0.y)
    }

    fn __repr__(&self) -> String {
        format!("Vec2({}, {})", self.0.x, self.0.y)
    }
}

#[pyclass(name = "Vec4", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyVec4(pub(crate) molgfx::math::Vec4);

#[pymethods]
impl PyVec4 {
    #[new]
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(molgfx::math::Vec4::new(x, y, z, w))
    }

    #[classattr]
    #[pyo3(name = "ZERO")]
    fn zero() -> Self {
        Self(molgfx::math::Vec4::ZERO)
    }

    #[getter]
    fn x(&self) -> f32 {
        self.0.x
    }

    #[getter]
    fn y(&self) -> f32 {
        self.0.y
    }

    #[getter]
    fn z(&self) -> f32 {
        self.0.z
    }

    #[getter]
    fn w(&self) -> f32 {
        self.0.w
    }

    #[pyo3(name = "to_tuple")]
    fn tuple(&self) -> (f32, f32, f32, f32) {
        (self.0.x, self.0.y, self.0.z, self.0.w)
    }

    fn __repr__(&self) -> String {
        format!(
            "Vec4({}, {}, {}, {})",
            self.0.x, self.0.y, self.0.z, self.0.w
        )
    }
}

#[pyclass(name = "Mat3", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyMat3(pub(crate) molgfx::math::Mat3);

#[pymethods]
impl PyMat3 {
    #[new]
    fn new(values: Vec<f32>) -> PyResult<Self> {
        let values: [f32; 9] = values
            .try_into()
            .map_err(|_| value("Mat3 requires exactly 9 column-major values"))?;
        let columns = [
            molgfx::math::Vec3::new(values[0], values[1], values[2]),
            molgfx::math::Vec3::new(values[3], values[4], values[5]),
            molgfx::math::Vec3::new(values[6], values[7], values[8]),
        ];
        Ok(Self(molgfx::math::Mat3::from_cols(
            columns[0], columns[1], columns[2],
        )))
    }

    #[staticmethod]
    fn identity() -> Self {
        Self(molgfx::math::Mat3::IDENTITY)
    }

    #[pyo3(name = "to_list")]
    fn list(&self) -> Vec<f32> {
        [
            self.0.x_axis.x,
            self.0.x_axis.y,
            self.0.x_axis.z,
            self.0.y_axis.x,
            self.0.y_axis.y,
            self.0.y_axis.z,
            self.0.z_axis.x,
            self.0.z_axis.y,
            self.0.z_axis.z,
        ]
        .to_vec()
    }

    fn __repr__(&self) -> String {
        format!("Mat3({})", self.list().len())
    }
}

/// One sampled point and unit tangent along a spline.
#[pyclass(name = "CurveSample", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyCurveSample(pub(crate) molgfx::math::CurveSample);

#[pymethods]
impl PyCurveSample {
    #[getter]
    fn position(&self) -> PyVec3 {
        PyVec3(self.0.position)
    }

    #[getter]
    fn tangent(&self) -> PyVec3 {
        PyVec3(self.0.tangent)
    }

    #[getter]
    fn segment(&self) -> u32 {
        self.0.segment
    }

    #[getter]
    fn parameter(&self) -> f32 {
        self.0.parameter
    }
}

/// An orthonormal frame carried along a sampled curve.
#[pyclass(name = "TransportFrame", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyTransportFrame(pub(crate) molgfx::math::TransportFrame);

#[pymethods]
impl PyTransportFrame {
    #[getter]
    fn tangent(&self) -> PyVec3 {
        PyVec3(self.0.tangent)
    }

    #[getter]
    fn normal(&self) -> PyVec3 {
        PyVec3(self.0.normal)
    }

    #[getter]
    fn binormal(&self) -> PyVec3 {
        PyVec3(self.0.binormal)
    }
}

/// Samples a Catmull-Rom trace, taking more points where the curve bends.
///
/// The evaluation runs in Rust; only the finished points cross into Python.
#[pyfunction]
#[pyo3(name = "sample_catmull_rom")]
pub(crate) fn sample_catmull_rom(
    points: Vec<PyVec3>,
    tolerance: f32,
    max_steps: u8,
) -> Vec<PyCurveSample> {
    let points: Vec<molgfx::math::Vec3> = points.into_iter().map(|point| point.0).collect();
    let mut out = Vec::new();
    molgfx::math::sample_catmull_rom(&points, tolerance, max_steps, &mut out);
    out.into_iter().map(PyCurveSample).collect()
}

/// Samples a Catmull-Rom trace, using `demand` to keep the count down where a
/// caller needs fewer points.
#[pyfunction]
#[pyo3(name = "sample_catmull_rom_demanding")]
pub(crate) fn sample_catmull_rom_demanding(
    points: Vec<PyVec3>,
    tolerance: f32,
    max_steps: u8,
    demand: Vec<f32>,
) -> Vec<PyCurveSample> {
    let points: Vec<molgfx::math::Vec3> = points.into_iter().map(|point| point.0).collect();
    let mut out = Vec::new();
    molgfx::math::sample_catmull_rom_demanding(&points, tolerance, max_steps, &demand, &mut out);
    out.into_iter().map(PyCurveSample).collect()
}

/// Samples a Catmull-Rom trace at a fixed number of steps per control interval.
#[pyfunction]
#[pyo3(name = "sample_catmull_rom_fixed")]
pub(crate) fn sample_catmull_rom_fixed(
    points: Vec<PyVec3>,
    steps_per_segment: u8,
) -> Vec<PyCurveSample> {
    let points: Vec<molgfx::math::Vec3> = points.into_iter().map(|point| point.0).collect();
    let mut out = Vec::new();
    molgfx::math::sample_catmull_rom_fixed(&points, steps_per_segment, &mut out);
    out.into_iter().map(PyCurveSample).collect()
}

/// Samples a cubic Bézier curve from its four control points.
#[pyfunction]
#[pyo3(name = "sample_cubic_bezier")]
pub(crate) fn sample_cubic_bezier(control: Vec<PyVec3>, segments: u16) -> PyResult<Vec<PyVec3>> {
    let control: [PyVec3; 4] = control
        .try_into()
        .map_err(|_| value("sample_cubic_bezier requires exactly 4 control points"))?;
    let control = [control[0].0, control[1].0, control[2].0, control[3].0];
    let mut out = Vec::new();
    molgfx::math::sample_cubic_bezier(control, segments, &mut out);
    Ok(out.into_iter().map(PyVec3).collect())
}

/// Builds minimal-rotation frames along a sampled curve.
#[pyfunction]
#[pyo3(name = "parallel_transport")]
pub(crate) fn parallel_transport(samples: Vec<PyCurveSample>) -> Vec<PyTransportFrame> {
    let samples: Vec<molgfx::math::CurveSample> =
        samples.into_iter().map(|sample| sample.0).collect();
    let mut out = Vec::new();
    molgfx::math::parallel_transport(&samples, &mut out);
    out.into_iter().map(PyTransportFrame).collect()
}

/// Packs a value in [0, 1] into an 8-bit unorm channel.
#[pyfunction]
#[pyo3(name = "unorm8")]
pub(crate) fn unorm8(value: f32) -> u8 {
    molgfx::math::unorm8(value)
}

/// Packs a value in [0, 255] into a byte, rounding to nearest.
#[pyfunction]
#[pyo3(name = "round_u8")]
pub(crate) fn round_u8(value: f32) -> u8 {
    molgfx::math::round_u8(value)
}

/// Truncates a value in [0, 65535] into a 16-bit unsigned integer.
#[pyfunction]
#[pyo3(name = "truncate_u16")]
pub(crate) fn truncate_u16(value: f32) -> u16 {
    molgfx::math::truncate_u16(value)
}

/// Quantizes a value in [0, 1] onto a grid of `levels` steps.
#[pyfunction]
#[pyo3(name = "unit_to_grid")]
pub(crate) fn unit_to_grid(value: f32, levels: u32) -> u32 {
    molgfx::math::unit_to_grid(value, levels)
}
