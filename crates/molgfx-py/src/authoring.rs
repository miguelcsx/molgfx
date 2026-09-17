//! Columnar Python authoring adapters for high-cardinality drawable data.

use crate::core::PyMaterial;
use crate::core::PyRelationPattern;
use crate::core::{PyMeshHandle, PyPrimitiveHandle, PyScene, PyStructureHandle};
use crate::error::{core, value};
use crate::math::PyRgba8;
use crate::values::PyCrystalCell;
use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;

#[pyclass(name = "ParticleShape", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyParticleShape {
    Sphere,
    Box,
    Cylinder,
    Spherocylinder,
    Gaussian,
    Circle,
    Square,
    Superquadric,
}

impl From<PyParticleShape> for molgfx::ParticleShape {
    fn from(value: PyParticleShape) -> Self {
        match value {
            PyParticleShape::Sphere => Self::Sphere,
            PyParticleShape::Box => Self::Box,
            PyParticleShape::Cylinder => Self::Cylinder,
            PyParticleShape::Spherocylinder => Self::Spherocylinder,
            PyParticleShape::Gaussian => Self::Gaussian,
            PyParticleShape::Circle => Self::Circle,
            PyParticleShape::Square => Self::Square,
            PyParticleShape::Superquadric => Self::Superquadric,
        }
    }
}

#[pyclass(name = "MeshTopology", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyMeshTopology {
    Triangles,
    TriangleStrip,
    TriangleFan,
    Quads,
}

impl From<PyMeshTopology> for molgfx::MeshTopology {
    fn from(value: PyMeshTopology) -> Self {
        match value {
            PyMeshTopology::Triangles => Self::Triangles,
            PyMeshTopology::TriangleStrip => Self::TriangleStrip,
            PyMeshTopology::TriangleFan => Self::TriangleFan,
            PyMeshTopology::Quads => Self::Quads,
        }
    }
}

#[pyclass(name = "GuideCap", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyGuideCap {
    Plain,
    Arrow,
    DoubleArrow,
}

impl From<PyGuideCap> for molgfx::GuideCap {
    fn from(value: PyGuideCap) -> Self {
        match value {
            PyGuideCap::Plain => Self::None,
            PyGuideCap::Arrow => Self::Arrow,
            PyGuideCap::DoubleArrow => Self::DoubleArrow,
        }
    }
}

#[pyclass(name = "GuideStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyGuideStyle(pub(crate) molgfx::GuideStyle);

#[pymethods]
impl PyGuideStyle {
    #[new]
    #[pyo3(signature = (color, pattern=PyRelationPattern::Solid, width_pixels=1.6, opacity=1.0, period_pixels=8.0, duty_cycle=0.5, cap=PyGuideCap::Plain, arrow_pixels=9.0))]
    fn new(
        color: PyRgba8,
        pattern: PyRelationPattern,
        width_pixels: f32,
        opacity: f32,
        period_pixels: f32,
        duty_cycle: f32,
        cap: PyGuideCap,
        arrow_pixels: f32,
    ) -> Self {
        Self(
            molgfx::GuideStyle {
                color: color.0,
                pattern: pattern.into(),
                width_pixels,
                opacity,
                period_pixels,
                duty_cycle,
                cap: cap.into(),
                arrow_pixels,
            }
            .sanitized(),
        )
    }
}

fn contiguous2<'a, T: numpy::Element>(
    values: &'a PyReadonlyArray2<'_, T>,
    columns: usize,
    label: &str,
) -> PyResult<&'a [T]> {
    if values.shape().get(1).copied() != Some(columns) {
        return Err(value(format!("{label} must have shape (N, {columns})")));
    }
    values
        .as_slice()
        .map_err(|_| value(format!("{label} must be C-contiguous")))
}

#[pymethods]
impl PyScene {
    fn copy_mesh_from_numpy(
        &mut self,
        owner: PyStructureHandle,
        positions: PyReadonlyArray2<'_, f32>,
        normals: PyReadonlyArray2<'_, f32>,
        colors: PyReadonlyArray2<'_, u8>,
        indices: PyReadonlyArray1<'_, u32>,
        material: PyMaterial,
    ) -> PyResult<PyMeshHandle> {
        let positions = contiguous2(&positions, 3, "positions")?;
        let normals = contiguous2(&normals, 3, "normals")?;
        let colors = contiguous2(&colors, 4, "colors")?;
        let count = positions.len() / 3;
        if normals.len() / 3 != count || colors.len() / 4 != count {
            return Err(value("mesh vertex columns must have the same row count"));
        }
        let indices = indices
            .as_slice()
            .map_err(|_| value("indices must be C-contiguous uint32"))?;
        let mut vertices = Vec::with_capacity(count);
        for row in 0..count {
            vertices.push(molgfx::MeshVertex {
                position: molgfx::Vec3::from_slice(&positions[row * 3..row * 3 + 3]),
                normal: molgfx::Vec3::from_slice(&normals[row * 3..row * 3 + 3]),
                color: molgfx::Rgba8::new(
                    colors[row * 4],
                    colors[row * 4 + 1],
                    colors[row * 4 + 2],
                    colors[row * 4 + 3],
                ),
            });
        }
        let mesh = core(molgfx::Mesh::new(
            owner.0,
            vertices,
            indices.to_vec(),
            material.0,
        ))?;
        core(self.inner.add_mesh(mesh)).map(Into::into)
    }

    fn copy_particles_from_numpy(
        &mut self,
        owner: PyStructureHandle,
        centers: PyReadonlyArray2<'_, f32>,
        sizes: PyReadonlyArray2<'_, f32>,
        orientations: PyReadonlyArray2<'_, f32>,
        colors: PyReadonlyArray2<'_, u8>,
        opacities: PyReadonlyArray1<'_, f32>,
        shape: PyParticleShape,
    ) -> PyResult<Option<PyPrimitiveHandle>> {
        let centers = contiguous2(&centers, 3, "centers")?;
        let sizes = contiguous2(&sizes, 3, "sizes")?;
        let orientations = contiguous2(&orientations, 4, "orientations")?;
        let colors = contiguous2(&colors, 4, "colors")?;
        let opacities = opacities
            .as_slice()
            .map_err(|_| value("opacities must be C-contiguous float32"))?;
        let count = centers.len() / 3;
        if sizes.len() / 3 != count
            || orientations.len() / 4 != count
            || colors.len() / 4 != count
            || opacities.len() != count
        {
            return Err(value("particle columns must have the same row count"));
        }
        let mut primitives = Vec::with_capacity(count);
        for row in 0..count {
            let particle = core(molgfx::Particle::new(
                owner.0,
                molgfx::Vec3::from_slice(&centers[row * 3..row * 3 + 3]),
                molgfx::Quat::from_xyzw(
                    orientations[row * 4],
                    orientations[row * 4 + 1],
                    orientations[row * 4 + 2],
                    orientations[row * 4 + 3],
                ),
                molgfx::Vec3::from_slice(&sizes[row * 3..row * 3 + 3]),
                shape.into(),
                molgfx::Rgba8::new(
                    colors[row * 4],
                    colors[row * 4 + 1],
                    colors[row * 4 + 2],
                    colors[row * 4 + 3],
                ),
                opacities[row],
            ))?;
            primitives.push(molgfx::Primitive::Particle(particle));
        }
        core(self.inner.add_primitives(&primitives)).map(|handle| handle.map(Into::into))
    }

    fn copy_polyline_from_numpy(
        &mut self,
        owner: PyStructureHandle,
        points: PyReadonlyArray2<'_, f32>,
        closed: bool,
        style: PyGuideStyle,
    ) -> PyResult<Vec<crate::core::PyGuideHandle>> {
        let points = contiguous2(&points, 3, "points")?;
        let points = points
            .chunks_exact(3)
            .map(molgfx::Vec3::from_slice)
            .collect::<Vec<_>>();
        let kind = if closed {
            molgfx::PolylineKind::Closed
        } else {
            molgfx::PolylineKind::Open
        };
        core(self.inner.add_polyline(owner.0, &points, kind, style.0))
            .map(|handles| handles.into_iter().map(Into::into).collect())
    }

    fn add_unit_cell(
        &mut self,
        owner: PyStructureHandle,
        cell: PyCrystalCell,
        style: PyGuideStyle,
    ) -> PyResult<Vec<crate::core::PyGuideHandle>> {
        core(self.inner.add_unit_cell(owner.0, cell.0, style.0))
            .map(|handles| handles.into_iter().map(Into::into).collect())
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyParticleShape>()?;
    module.add_class::<PyMeshTopology>()?;
    module.add_class::<PyGuideCap>()?;
    module.add_class::<PyGuideStyle>()
}
