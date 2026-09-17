//! Python adapters for clipping, segmentation styles and crystallography.

use crate::error::core;
use crate::math::{PyMat4, PyRgba8, PyVec3};
use pyo3::prelude::*;

#[pyclass(name = "VolumeStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVolumeStyle(pub(crate) molgfx::VolumeStyle);

#[pymethods]
impl PyVolumeStyle {
    #[staticmethod]
    fn direct() -> Self {
        Self(molgfx::VolumeStyle::default())
    }
    #[staticmethod]
    fn isosurface() -> Self {
        Self(molgfx::VolumeStyle::isosurface())
    }
    #[staticmethod]
    fn medium() -> Self {
        Self(molgfx::VolumeStyle::medium())
    }
    #[staticmethod]
    fn liquid_surface() -> Self {
        Self(molgfx::VolumeStyle::liquid_surface())
    }
    #[staticmethod]
    fn slice(plane: PyClipPlane) -> Self {
        Self(molgfx::VolumeStyle::slice(molgfx::VolumeSlice::new(
            plane.0,
        )))
    }
    fn region(
        &self,
        minimum: (u32, u32, u32),
        maximum: (u32, u32, u32),
        dimensions: (u32, u32, u32),
    ) -> PyResult<Self> {
        core(molgfx::VolumeRegion::new(
            [minimum.0, minimum.1, minimum.2],
            [maximum.0, maximum.1, maximum.2],
            [dimensions.0, dimensions.1, dimensions.2],
        ))
        .map(|region| Self(self.0.region(region)))
    }
    fn sampling(&self, opacity_scale: f32, step_scale: f32) -> Self {
        Self(self.0.sampling(opacity_scale, step_scale))
    }
}

#[pyclass(name = "ClipCap", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyClipCap {
    Open,
    Solid,
}

impl From<PyClipCap> for molgfx::ClipCap {
    fn from(value: PyClipCap) -> Self {
        match value {
            PyClipCap::Open => Self::Open,
            PyClipCap::Solid => Self::Solid,
        }
    }
}

#[pyclass(name = "ClipPlane", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyClipPlane(pub(crate) molgfx::ClipPlane);

#[pymethods]
impl PyClipPlane {
    #[staticmethod]
    fn from_point_normal(point: PyVec3, normal: PyVec3) -> PyResult<Self> {
        core(molgfx::ClipPlane::from_point_normal(point.0, normal.0)).map(Self)
    }
    #[getter]
    fn normal(&self) -> PyVec3 {
        PyVec3(self.0.normal)
    }
    #[getter]
    fn offset(&self) -> f32 {
        self.0.offset
    }
    fn reversed(&self) -> Self {
        Self(self.0.reversed())
    }
    fn signed_distance(&self, point: PyVec3) -> f32 {
        self.0.signed_distance(point.0)
    }
}

#[pyclass(name = "ClipSet", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyClipSet(pub(crate) molgfx::ClipSet);

#[pymethods]
impl PyClipSet {
    #[new]
    fn new(planes: Vec<PyClipPlane>) -> PyResult<Self> {
        let planes = planes.into_iter().map(|plane| plane.0).collect::<Vec<_>>();
        core(molgfx::ClipSet::new(&planes)).map(Self)
    }
    #[staticmethod]
    fn slab(center: PyVec3, normal: PyVec3, thickness: f32) -> PyResult<Self> {
        core(molgfx::ClipSet::slab(center.0, normal.0, thickness)).map(Self)
    }
    fn with_cap(&self, cap: PyClipCap) -> Self {
        Self(self.0.with_cap(cap.into()))
    }
    #[getter]
    fn cap(&self) -> PyClipCap {
        match self.0.cap() {
            molgfx::ClipCap::Open => PyClipCap::Open,
            molgfx::ClipCap::Solid => PyClipCap::Solid,
        }
    }
    #[getter]
    fn plane_count(&self) -> usize {
        self.0.planes().len()
    }
    fn contains(&self, point: PyVec3) -> bool {
        self.0.contains(point.0)
    }
}

#[pyclass(name = "SegmentStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySegmentStyle(pub(crate) molgfx::SegmentStyle);

#[pymethods]
impl PySegmentStyle {
    #[new]
    fn new(label: u32, color: PyRgba8, opacity: f32) -> Self {
        Self(molgfx::SegmentStyle::new(label, color.0, opacity))
    }
    #[getter]
    fn label(&self) -> u32 {
        self.0.label
    }
    #[getter]
    fn color(&self) -> PyRgba8 {
        self.0.color.into()
    }
    #[getter]
    fn opacity(&self) -> f32 {
        self.0.opacity
    }
}

#[pyclass(name = "SegmentStyleTable", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySegmentStyleTable(pub(crate) molgfx::SegmentStyleTable);

#[pymethods]
impl PySegmentStyleTable {
    #[new]
    fn new(styles: Vec<PySegmentStyle>) -> PyResult<Self> {
        let styles = styles.into_iter().map(|style| style.0).collect::<Vec<_>>();
        core(molgfx::SegmentStyleTable::new(&styles)).map(Self)
    }
    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::SegmentStyleTable::default())
    }
    fn style_for(&self, label: u32) -> Option<PySegmentStyle> {
        self.0.style_for(label).map(PySegmentStyle)
    }
    #[getter]
    fn len(&self) -> usize {
        self.0.styles().len()
    }
}

#[pyclass(name = "SegmentationStyle", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySegmentationStyle(pub(crate) molgfx::SegmentationStyle);

#[pymethods]
impl PySegmentationStyle {
    #[new]
    #[pyo3(signature = (styles=None, opacity_scale=1.0, step_scale=0.65))]
    fn new(styles: Option<PySegmentStyleTable>, opacity_scale: f32, step_scale: f32) -> Self {
        Self(molgfx::SegmentationStyle {
            styles: styles.map_or_else(molgfx::SegmentStyleTable::default, |value| value.0),
            opacity_scale,
            step_scale,
            ..molgfx::SegmentationStyle::default()
        })
    }
    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::SegmentationStyle::default())
    }
    #[getter]
    fn styles(&self) -> PySegmentStyleTable {
        PySegmentStyleTable(self.0.styles.clone())
    }
    #[getter]
    fn opacity_scale(&self) -> f32 {
        self.0.opacity_scale
    }
    #[getter]
    fn step_scale(&self) -> f32 {
        self.0.step_scale
    }
}

#[pyclass(name = "TubeRadiusMapping", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyTubeRadiusMapping(pub(crate) molgfx::TubeRadiusMapping);

#[pymethods]
impl PyTubeRadiusMapping {
    #[staticmethod]
    fn constant() -> Self {
        Self(molgfx::TubeRadiusMapping::Constant)
    }
    #[staticmethod]
    fn b_factor(domain: (f32, f32), radii: (f32, f32)) -> PyResult<Self> {
        core(molgfx::TubeRadiusMapping::b_factor(
            [domain.0, domain.1],
            [radii.0, radii.1],
        ))
        .map(Self)
    }
    fn radius(&self, value: f32, fallback: f32) -> f32 {
        self.0.radius(value, fallback)
    }
    fn value(&self, radius: f32) -> Option<f32> {
        self.0.value(radius)
    }
}

#[pyclass(name = "CrystalCell", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyCrystalCell(pub(crate) molgfx::CrystalCell);

#[pymethods]
impl PyCrystalCell {
    #[new]
    fn new(lengths: (f32, f32, f32), angles_degrees: (f32, f32, f32)) -> PyResult<Self> {
        core(molgfx::CrystalCell::new(
            [lengths.0, lengths.1, lengths.2],
            [angles_degrees.0, angles_degrees.1, angles_degrees.2],
        ))
        .map(Self)
    }
    fn with_origin(&self, origin: PyVec3) -> Self {
        Self(self.0.with_origin(origin.0))
    }
    #[getter]
    fn lengths(&self) -> (f32, f32, f32) {
        let value = self.0.lengths();
        (value[0], value[1], value[2])
    }
    #[getter]
    fn angles_degrees(&self) -> (f32, f32, f32) {
        let value = self.0.angles_degrees();
        (value[0], value[1], value[2])
    }
    #[getter]
    fn origin(&self) -> PyVec3 {
        PyVec3(self.0.origin())
    }
}

#[pyclass(name = "SymmetryInstance", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySymmetryInstance(pub(crate) molgfx::SymmetryInstance);

#[pymethods]
impl PySymmetryInstance {
    #[new]
    fn new(id: u32, transform: PyMat4) -> PyResult<Self> {
        core(molgfx::SymmetryInstance::new(id, transform.0)).map(Self)
    }
    #[getter]
    fn id(&self) -> u32 {
        self.0.id
    }
    #[getter]
    fn transform(&self) -> PyMat4 {
        PyMat4(self.0.transform)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyVolumeStyle>()?;
    module.add_class::<PyClipCap>()?;
    module.add_class::<PyClipPlane>()?;
    module.add_class::<PyClipSet>()?;
    module.add_class::<PySegmentStyle>()?;
    module.add_class::<PySegmentStyleTable>()?;
    module.add_class::<PySegmentationStyle>()?;
    module.add_class::<PyTubeRadiusMapping>()?;
    module.add_class::<PyCrystalCell>()?;
    module.add_class::<PySymmetryInstance>()
}
