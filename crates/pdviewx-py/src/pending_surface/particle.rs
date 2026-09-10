//! Bounded visual-particle motion adapters.

use crate::error::core;
use crate::math::{PyAabb, PyVec3};
use pyo3::prelude::*;

#[pyclass(name = "ParticleBoundary", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyParticleBoundary {
    Bounce,
    Wrap,
}

impl From<PyParticleBoundary> for pdviewx::ParticleBoundary {
    fn from(value: PyParticleBoundary) -> Self {
        match value {
            PyParticleBoundary::Bounce => Self::Bounce,
            PyParticleBoundary::Wrap => Self::Wrap,
        }
    }
}

impl From<pdviewx::ParticleBoundary> for PyParticleBoundary {
    fn from(value: pdviewx::ParticleBoundary) -> Self {
        match value {
            pdviewx::ParticleBoundary::Bounce => Self::Bounce,
            pdviewx::ParticleBoundary::Wrap => Self::Wrap,
        }
    }
}

#[pyclass(name = "ParticleMotion", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyParticleMotion(pub(crate) pdviewx::ParticleMotion);

#[pymethods]
impl PyParticleMotion {
    #[new]
    #[pyo3(signature = (velocity, bounds, fixed_timestep, seed, boundary=PyParticleBoundary::Bounce, respawn_after_steps=0))]
    fn new(
        velocity: PyVec3,
        bounds: PyAabb,
        fixed_timestep: f32,
        seed: u32,
        boundary: PyParticleBoundary,
        respawn_after_steps: u32,
    ) -> PyResult<Self> {
        core(pdviewx::ParticleMotion::new(
            velocity.0,
            bounds.0,
            fixed_timestep,
            seed,
            boundary.into(),
        ))
        .map(|motion| Self(motion.with_respawn_after_steps(respawn_after_steps)))
    }

    #[getter]
    fn velocity(&self) -> PyVec3 {
        PyVec3(self.0.velocity())
    }

    #[getter]
    fn bounds(&self) -> PyAabb {
        PyAabb(self.0.bounds())
    }

    #[getter]
    fn fixed_timestep(&self) -> f32 {
        self.0.fixed_timestep()
    }

    #[getter]
    fn seed(&self) -> u32 {
        self.0.seed()
    }

    #[getter]
    fn boundary(&self) -> PyParticleBoundary {
        self.0.boundary().into()
    }

    #[getter]
    fn respawn_after_steps(&self) -> u32 {
        self.0.respawn_after_steps()
    }
}
