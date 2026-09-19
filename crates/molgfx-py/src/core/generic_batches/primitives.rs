//! Caller-authored particles and the heterogeneous primitive table.

use super::super::{PyEntityRef, PyPrimitiveHandle, PyScene, PyStructureHandle};
use crate::authoring::PyParticleShape;
use crate::error::core;
use crate::math::{PyAabb, PyQuat, PyRgba8, PyVec3};
use crate::pending_surface::geometry::{
    PyAnisotropicEllipsoid, PyCarbohydrateSymbol, PyPlanarRegion,
};
use crate::pending_surface::particle::PyParticleMotion;
use pyo3::prelude::*;

/// One caller-authored particle glyph.
#[pyclass(name = "Particle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyParticle(pub(crate) molgfx::core::Particle);

#[pymethods]
impl PyParticle {
    #[new]
    #[pyo3(signature = (owner, center, orientation, size, shape, color=None, opacity=1.0))]
    fn new(
        owner: PyStructureHandle,
        center: PyVec3,
        orientation: PyQuat,
        size: PyVec3,
        shape: PyParticleShape,
        color: Option<PyRgba8>,
        opacity: f32,
    ) -> PyResult<Self> {
        core(molgfx::core::Particle::new(
            owner.0,
            center.0,
            orientation.0,
            size.0,
            shape.into(),
            match color {
                Some(color) => color.0,
                None => molgfx::math::Rgba8::WHITE,
            },
            opacity,
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
    fn shape(&self) -> PyParticleShape {
        self.0.shape.into()
    }

    #[getter]
    fn shape_parameters(&self) -> [f32; 2] {
        self.0.shape_parameters
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        PyRgba8(self.0.color)
    }

    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }

    #[getter]
    fn motion(&self) -> Option<PyParticleMotion> {
        self.0.motion.map(PyParticleMotion)
    }

    #[getter]
    fn bounds(&self) -> PyAabb {
        PyAabb(self.0.bounds())
    }

    /// Sets the north-south and east-west superquadric exponents.
    fn with_superquadric_exponents(&self, latitude: f32, longitude: f32) -> PyResult<Self> {
        core(self.0.with_superquadric_exponents(latitude, longitude)).map(Self)
    }

    /// Enables bounded visual advection without moving the source pose.
    fn with_motion(&self, motion: PyParticleMotion) -> Self {
        Self(self.0.with_motion(motion.0))
    }

    /// Removes visual advection and keeps the current source pose.
    fn without_motion(&self) -> Self {
        Self(self.0.without_motion())
    }

    fn __repr__(&self) -> String {
        format!(
            "Particle(shape={:?}, center={:?})",
            self.0.shape, self.0.center
        )
    }
}

/// One entry of the shared analytic primitive table.
///
/// The table draws heterogeneous caller geometry with one indirect call, so an
/// entry carries whichever of the four payloads it was built from and leaves
/// the other three empty.
#[pyclass(name = "Primitive", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPrimitive(pub(crate) molgfx::core::Primitive);

impl From<molgfx::core::Primitive> for PyPrimitive {
    fn from(value: molgfx::core::Primitive) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyPrimitive {
    #[getter]
    fn owner(&self) -> PyStructureHandle {
        self.0.owner().into()
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible()
    }

    #[getter]
    fn bounds(&self) -> PyAabb {
        PyAabb(self.0.bounds())
    }

    #[getter]
    fn particle(&self) -> Option<PyParticle> {
        match self.0 {
            molgfx::core::Primitive::Particle(value) => Some(PyParticle(value)),
            _ => None,
        }
    }

    #[getter]
    fn ellipsoid(&self) -> Option<PyAnisotropicEllipsoid> {
        match self.0 {
            molgfx::core::Primitive::Ellipsoid { value, .. } => Some(PyAnisotropicEllipsoid(value)),
            _ => None,
        }
    }

    #[getter]
    fn carbohydrate(&self) -> Option<PyCarbohydrateSymbol> {
        match self.0 {
            molgfx::core::Primitive::Carbohydrate(value) => Some(PyCarbohydrateSymbol(value)),
            _ => None,
        }
    }

    #[getter]
    fn planar(&self) -> Option<PyPlanarRegion> {
        match self.0 {
            molgfx::core::Primitive::Planar { value, .. } => Some(PyPlanarRegion(value)),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Primitive(owner={:?}, visible={})",
            self.0.owner(),
            self.0.visible()
        )
    }
}

#[pymethods]
impl PyScene {
    /// Resolves one primitive from the shared analytic table.
    fn primitive(&self, handle: PyPrimitiveHandle) -> Option<PyPrimitive> {
        self.inner.primitive(handle.0).copied().map(Into::into)
    }

    /// Iterates active and hidden primitives in stable slot order.
    fn primitives(&self) -> Vec<(PyPrimitiveHandle, PyPrimitive)> {
        self.inner
            .primitives()
            .map(|(handle, value)| (handle.into(), (*value).into()))
            .collect()
    }

    /// Resolves a picked primitive row without scanning the table.
    fn primitive_for_entity(&self, entity: PyEntityRef) -> Option<PyPrimitive> {
        self.inner
            .primitive_for_entity(entity.0)
            .copied()
            .map(Into::into)
    }

    /// Removes a primitive and invalidates its handle.
    fn remove_primitive(&mut self, handle: PyPrimitiveHandle) -> Option<PyPrimitive> {
        self.inner.remove_primitive(handle.0).map(Into::into)
    }
}
