//! Typed adapters for caller-authored analytic geometry.

use crate::authoring::PyGuideStyle;
use crate::core::PyStructureHandle;
use crate::error::core;
use crate::math::{PyAabb, PyQuat, PyRgba8, PyVec3};
use pyo3::prelude::*;

#[pyclass(name = "AnisotropicEllipsoid", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyAnisotropicEllipsoid(pub(crate) molgfx::AnisotropicEllipsoid);

#[pymethods]
impl PyAnisotropicEllipsoid {
    #[new]
    fn new(center: PyVec3, tensor: [f32; 6]) -> PyResult<Self> {
        core(molgfx::AnisotropicEllipsoid::new(center.0, tensor)).map(Self)
    }

    #[getter]
    fn center(&self) -> PyVec3 {
        PyVec3(self.0.center())
    }

    #[getter]
    fn tensor(&self) -> [f32; 6] {
        self.0.tensor()
    }

    #[getter]
    fn inverse_tensor(&self) -> Option<[f32; 6]> {
        self.0.inverse_tensor()
    }

    #[getter]
    fn bounds(&self) -> PyAabb {
        PyAabb(self.0.bounds())
    }
}

#[pyclass(name = "CarbohydrateShape", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyCarbohydrateShape {
    Unknown,
    Glc,
    Gal,
    Man,
    Fuc,
    Xyl,
    Neu5Ac,
}

impl From<PyCarbohydrateShape> for molgfx::CarbohydrateShape {
    fn from(value: PyCarbohydrateShape) -> Self {
        match value {
            PyCarbohydrateShape::Unknown => Self::Unknown,
            PyCarbohydrateShape::Glc => Self::Glc,
            PyCarbohydrateShape::Gal => Self::Gal,
            PyCarbohydrateShape::Man => Self::Man,
            PyCarbohydrateShape::Fuc => Self::Fuc,
            PyCarbohydrateShape::Xyl => Self::Xyl,
            PyCarbohydrateShape::Neu5Ac => Self::Neu5Ac,
        }
    }
}

impl From<molgfx::CarbohydrateShape> for PyCarbohydrateShape {
    fn from(value: molgfx::CarbohydrateShape) -> Self {
        match value {
            molgfx::CarbohydrateShape::Glc => Self::Glc,
            molgfx::CarbohydrateShape::Gal => Self::Gal,
            molgfx::CarbohydrateShape::Man => Self::Man,
            molgfx::CarbohydrateShape::Fuc => Self::Fuc,
            molgfx::CarbohydrateShape::Xyl => Self::Xyl,
            molgfx::CarbohydrateShape::Neu5Ac => Self::Neu5Ac,
            _ => Self::Unknown,
        }
    }
}

#[pyclass(name = "CarbohydrateSymbol", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyCarbohydrateSymbol(pub(crate) molgfx::CarbohydrateSymbol);

#[pymethods]
impl PyCarbohydrateSymbol {
    #[new]
    fn new(
        owner: PyStructureHandle,
        center: PyVec3,
        orientation: PyQuat,
        size: PyVec3,
        shape: PyCarbohydrateShape,
        color: PyRgba8,
    ) -> PyResult<Self> {
        core(molgfx::CarbohydrateSymbol::new(
            owner.0,
            center.0,
            orientation.0,
            size.0,
            shape.into(),
            color.0,
        ))
        .map(Self)
    }

    #[getter]
    fn owner(&self) -> PyStructureHandle {
        self.0.owner.into()
    }

    #[getter]
    fn center(&self) -> PyVec3 {
        PyVec3(self.0.center)
    }

    #[getter]
    fn orientation(&self) -> PyQuat {
        PyQuat(self.0.orientation)
    }

    #[getter]
    fn size(&self) -> PyVec3 {
        PyVec3(self.0.size)
    }

    #[getter]
    fn shape(&self) -> PyCarbohydrateShape {
        self.0.shape.into()
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        self.0.color.into()
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }

    #[getter]
    fn bounds(&self) -> PyAabb {
        PyAabb(self.0.bounds())
    }
}

#[pyclass(name = "PlanarRegion", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPlanarRegion(pub(crate) molgfx::PlanarRegion);

#[pymethods]
impl PyPlanarRegion {
    #[new]
    fn new(
        owner: PyStructureHandle,
        center: PyVec3,
        normal: PyVec3,
        tangent: PyVec3,
        size: [f32; 2],
    ) -> PyResult<Self> {
        core(molgfx::PlanarRegion::new(
            owner.0, center.0, normal.0, tangent.0, size,
        ))
        .map(Self)
    }

    #[getter]
    fn owner(&self) -> PyStructureHandle {
        self.0.owner.into()
    }

    #[getter]
    fn center(&self) -> PyVec3 {
        PyVec3(self.0.center)
    }

    #[getter]
    fn normal(&self) -> PyVec3 {
        PyVec3(self.0.normal)
    }

    #[getter]
    fn tangent(&self) -> PyVec3 {
        PyVec3(self.0.tangent)
    }

    #[getter]
    fn bitangent(&self) -> PyVec3 {
        PyVec3(self.0.bitangent)
    }

    #[getter]
    fn size(&self) -> [f32; 2] {
        self.0.size
    }
}

#[pyclass(name = "Quadric", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyQuadric(pub(crate) molgfx::Quadric);

#[pymethods]
impl PyQuadric {
    #[new]
    fn new(coefficients: [f32; 10], bounds: PyAabb, cells: [u16; 3]) -> PyResult<Self> {
        core(molgfx::Quadric::new(coefficients, bounds.0, cells)).map(Self)
    }

    #[getter]
    fn coefficients(&self) -> [f32; 10] {
        self.0.coefficients()
    }

    #[getter]
    fn bounds(&self) -> PyAabb {
        PyAabb(self.0.bounds())
    }
}

#[pyclass(name = "Guide", from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyGuide(pub(crate) molgfx::Guide);

#[pymethods]
impl PyGuide {
    #[new]
    fn new(
        owner: PyStructureHandle,
        start: PyVec3,
        end: PyVec3,
        style: PyGuideStyle,
    ) -> PyResult<Self> {
        core(molgfx::Guide::new(owner.0, start.0, end.0, style.0)).map(Self)
    }

    #[getter]
    fn owner(&self) -> PyStructureHandle {
        self.0.owner().into()
    }

    #[getter]
    fn start(&self) -> PyVec3 {
        PyVec3(self.0.start())
    }

    #[getter]
    fn end(&self) -> PyVec3 {
        PyVec3(self.0.end())
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible()
    }

    fn set_visible(&mut self, visible: bool) {
        self.0.set_visible(visible);
    }

    fn set_style(&mut self, style: PyGuideStyle) {
        self.0.set_style(style.0);
    }
}
