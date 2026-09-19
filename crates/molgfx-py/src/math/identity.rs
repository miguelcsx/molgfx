//! The value identities: vectors, a quaternion, matrices, bounds and a camera.

use crate::error::value;
use pyo3::prelude::*;

#[pyclass(name = "Vec3", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyVec3(pub(crate) molgfx::math::Vec3);

#[pymethods]
impl PyVec3 {
    #[new]
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self(molgfx::math::Vec3::new(x, y, z))
    }

    #[classattr]
    #[pyo3(name = "ZERO")]
    fn zero() -> Self {
        Self(molgfx::math::Vec3::ZERO)
    }

    #[classattr]
    #[pyo3(name = "X")]
    fn x_axis() -> Self {
        Self(molgfx::math::Vec3::X)
    }

    #[classattr]
    #[pyo3(name = "Y")]
    fn y_axis() -> Self {
        Self(molgfx::math::Vec3::Y)
    }

    #[classattr]
    #[pyo3(name = "Z")]
    fn z_axis() -> Self {
        Self(molgfx::math::Vec3::Z)
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

    #[pyo3(name = "to_tuple")]
    fn tuple(&self) -> (f32, f32, f32) {
        (self.0.x, self.0.y, self.0.z)
    }

    fn __repr__(&self) -> String {
        format!("Vec3({}, {}, {})", self.0.x, self.0.y, self.0.z)
    }
}

#[pyclass(name = "Quat", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyQuat(pub(crate) molgfx::math::Quat);

#[pymethods]
impl PyQuat {
    #[new]
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(molgfx::math::Quat::from_xyzw(x, y, z, w))
    }

    #[staticmethod]
    fn identity() -> Self {
        Self(molgfx::math::Quat::IDENTITY)
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
}

#[pyclass(name = "Mat4", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PyMat4(pub(crate) molgfx::math::Mat4);

#[pymethods]
impl PyMat4 {
    #[new]
    fn new(values: Vec<f32>) -> PyResult<Self> {
        let values: [f32; 16] = values
            .try_into()
            .map_err(|_| value("Mat4 requires exactly 16 column-major values"))?;
        Ok(Self(molgfx::math::Mat4::from_cols_array(&values)))
    }

    #[staticmethod]
    fn identity() -> Self {
        Self(molgfx::math::Mat4::IDENTITY)
    }

    #[pyo3(name = "to_list")]
    fn list(&self) -> Vec<f32> {
        self.0.to_cols_array().to_vec()
    }

    fn __repr__(&self) -> String {
        format!("Mat4({:?})", self.0.to_cols_array())
    }
}

#[pyclass(name = "Rgba8", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyRgba8(pub(crate) molgfx::math::Rgba8);

impl From<molgfx::math::Rgba8> for PyRgba8 {
    fn from(value: molgfx::math::Rgba8) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyRgba8 {
    #[new]
    fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(molgfx::math::Rgba8::new(r, g, b, a))
    }

    #[staticmethod]
    fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self(molgfx::math::Rgba8::opaque(r, g, b))
    }

    #[classattr]
    #[pyo3(name = "WHITE")]
    fn white() -> Self {
        Self(molgfx::math::Rgba8::WHITE)
    }

    #[getter]
    fn r(&self) -> u8 {
        self.0.r
    }

    #[getter]
    fn g(&self) -> u8 {
        self.0.g
    }

    #[getter]
    fn b(&self) -> u8 {
        self.0.b
    }

    #[getter]
    fn a(&self) -> u8 {
        self.0.a
    }

    #[pyo3(name = "to_f32")]
    fn f32_components(&self) -> [f32; 4] {
        self.0.to_f32()
    }

    fn __repr__(&self) -> String {
        format!(
            "Rgba8({}, {}, {}, {})",
            self.0.r, self.0.g, self.0.b, self.0.a
        )
    }
}

#[pyclass(name = "Aabb", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyAabb(pub(crate) molgfx::math::Aabb);

#[pymethods]
impl PyAabb {
    #[new]
    fn new(min: PyVec3, max: PyVec3) -> Self {
        Self(molgfx::math::Aabb::new(min.0, max.0))
    }

    #[classattr]
    #[pyo3(name = "EMPTY")]
    fn empty() -> Self {
        Self(molgfx::math::Aabb::EMPTY)
    }

    #[getter]
    fn min(&self) -> PyVec3 {
        PyVec3(self.0.min)
    }

    #[getter]
    fn max(&self) -> PyVec3 {
        PyVec3(self.0.max)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn center(&self) -> PyVec3 {
        PyVec3(self.0.center())
    }

    fn bounding_sphere(&self) -> PyBoundingSphere {
        PyBoundingSphere(self.0.bounding_sphere())
    }
}

#[pyclass(name = "BoundingSphere", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyBoundingSphere(pub(crate) molgfx::math::BoundingSphere);

#[pymethods]
impl PyBoundingSphere {
    #[new]
    fn new(center: PyVec3, radius: f32) -> Self {
        Self(molgfx::math::BoundingSphere {
            center: center.0,
            radius,
        })
    }

    #[getter]
    fn center(&self) -> PyVec3 {
        PyVec3(self.0.center)
    }

    #[getter]
    fn radius(&self) -> f32 {
        self.0.radius
    }
}

#[pyclass(name = "Projection", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyProjection(pub(crate) molgfx::math::Projection);

#[pymethods]
impl PyProjection {
    #[staticmethod]
    #[pyo3(signature = (fov_y, aspect, near=0.1, far=1000.0))]
    fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        Self(molgfx::math::Projection::Perspective {
            fov_y,
            aspect,
            near,
            far,
        })
    }

    #[staticmethod]
    #[pyo3(signature = (height, aspect, near=0.1, far=1000.0))]
    fn orthographic(height: f32, aspect: f32, near: f32, far: f32) -> Self {
        Self(molgfx::math::Projection::Orthographic {
            height,
            aspect,
            near,
            far,
        })
    }

    #[getter]
    fn aspect(&self) -> f32 {
        self.0.aspect()
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            molgfx::math::Projection::Perspective { .. } => "perspective",
            molgfx::math::Projection::Orthographic { .. } => "orthographic",
        }
    }

    fn __repr__(&self) -> String {
        format!("Projection.{}(aspect={})", self.kind(), self.0.aspect())
    }
}

#[pyclass(name = "Camera", from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyCamera {
    pub(crate) inner: molgfx::math::Camera,
}

#[pymethods]
impl PyCamera {
    #[new]
    fn new(eye: PyVec3, target: PyVec3, up: PyVec3, projection: PyProjection) -> Self {
        Self {
            inner: molgfx::math::Camera {
                eye: eye.0,
                target: target.0,
                up: up.0,
                projection: projection.0,
            },
        }
    }

    #[staticmethod]
    fn framing(bound: PyBoundingSphere, aspect: f32) -> Self {
        Self {
            inner: molgfx::math::Camera::framing(&bound.0, aspect),
        }
    }

    #[staticmethod]
    fn framing_aabb(bound: PyAabb, aspect: f32) -> Self {
        Self {
            inner: molgfx::math::Camera::framing_aabb(&bound.0, aspect),
        }
    }

    #[staticmethod]
    fn look_at(eye: PyVec3, target: PyVec3, up: PyVec3) -> PyMat4 {
        PyMat4(molgfx::math::Camera::look_at(eye.0, target.0, up.0))
    }

    #[getter]
    fn eye(&self) -> PyVec3 {
        PyVec3(self.inner.eye)
    }

    #[getter]
    fn target(&self) -> PyVec3 {
        PyVec3(self.inner.target)
    }

    #[getter]
    fn up(&self) -> PyVec3 {
        PyVec3(self.inner.up)
    }

    #[getter]
    fn projection(&self) -> PyProjection {
        PyProjection(self.inner.projection)
    }

    #[getter]
    fn focus_distance(&self) -> f32 {
        self.inner.focus_distance()
    }

    fn view_proj(&self) -> PyMat4 {
        PyMat4(self.inner.view_proj())
    }
}
