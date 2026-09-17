//! Owned manifest-record adapters. Vector inputs use explicit `copy_*` names.

use crate::math::PyRgba8;
use crate::pending_surface::particle::{PyParticleBoundary, PyParticleMotion};
use pyo3::prelude::*;

const fn rgba(color: molgfx::Rgba8) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

#[pyclass(name = "ParticleMotionDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyParticleMotionDescription(pub(crate) molgfx::ParticleMotionDescription);

impl From<PyParticleMotion> for PyParticleMotionDescription {
    fn from(value: PyParticleMotion) -> Self {
        let bounds = value.0.bounds();
        Self(molgfx::ParticleMotionDescription {
            velocity: value.0.velocity().to_array(),
            bounds: [bounds.min.to_array(), bounds.max.to_array()],
            fixed_timestep: value.0.fixed_timestep(),
            seed: value.0.seed(),
            boundary: match value.0.boundary() {
                molgfx::ParticleBoundary::Bounce => "bounce",
                molgfx::ParticleBoundary::Wrap => "wrap",
            }
            .to_owned(),
            respawn_after_steps: value.0.respawn_after_steps(),
        })
    }
}

#[pymethods]
impl PyParticleMotionDescription {
    #[new]
    fn new(motion: PyParticleMotion) -> Self {
        motion.into()
    }

    #[getter]
    fn boundary(&self) -> PyParticleBoundary {
        if self.0.boundary == "wrap" {
            PyParticleBoundary::Wrap
        } else {
            PyParticleBoundary::Bounce
        }
    }
}

#[pyclass(name = "MeshDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyMeshDescription(pub(crate) molgfx::MeshDescription);

#[pymethods]
impl PyMeshDescription {
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    #[getter]
    fn content_hash(&self) -> u64 {
        self.0.content_hash
    }

    #[getter]
    fn face_visibility(&self) -> String {
        self.0.face_visibility.clone()
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }
}

#[pyclass(name = "MeshInstanceDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyMeshInstanceDescription(pub(crate) molgfx::MeshInstanceDescription);

#[pymethods]
impl PyMeshInstanceDescription {
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    #[getter]
    fn transform(&self) -> [f32; 16] {
        self.0.transform
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }
}

#[pyclass(name = "OverlayDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyOverlayDescription(pub(crate) molgfx::OverlayDescription);

#[pymethods]
impl PyOverlayDescription {
    #[staticmethod]
    #[pyo3(signature = (row, generation, kind, normalized, pixels, order, text, colors, values, visible=true))]
    #[allow(clippy::too_many_arguments)]
    fn copy_from_values(
        row: u32,
        generation: u32,
        kind: String,
        normalized: [f32; 2],
        pixels: [f32; 2],
        order: i16,
        text: String,
        colors: [PyRgba8; 2],
        values: [f32; 4],
        visible: bool,
    ) -> Self {
        Self(molgfx::OverlayDescription {
            row,
            generation,
            kind,
            normalized,
            pixels,
            order,
            visible,
            text,
            colors: colors.map(|color| rgba(color.0)),
            values,
        })
    }

    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }
}

#[pyclass(name = "PrimitiveDescription", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyPrimitiveDescription(pub(crate) molgfx::PrimitiveDescription);

#[pymethods]
impl PyPrimitiveDescription {
    #[getter]
    fn row(&self) -> u32 {
        self.0.row
    }

    #[getter]
    fn generation(&self) -> u32 {
        self.0.generation
    }

    #[getter]
    fn kind(&self) -> String {
        self.0.kind.clone()
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible
    }

    #[getter]
    fn color(&self) -> [u8; 4] {
        self.0.color
    }

    #[getter]
    fn center(&self) -> [f32; 3] {
        self.0.center
    }
}
