//! Exact representation recipe and target adapters.

use crate::core::{
    PyRepresentationKind, PySegmentationHandle, PySelect, PySelectionHandle, PyVolumeHandle,
};
use crate::error::core;
use crate::pending_surface::presentation::PySurfaceComponentPolicy;
use crate::values::{PySurfaceKind, PySurfaceStyle, PyTubeRadiusMapping};
use pyo3::prelude::*;

#[pyclass(name = "RepresentationInput", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRepresentationInput(pub(crate) molgfx::core::RepresentationInput);

#[pymethods]
impl PyRepresentationInput {
    #[staticmethod]
    fn stored(selection: PySelectionHandle) -> Self {
        Self(molgfx::core::RepresentationInput::Stored(selection.0))
    }

    #[staticmethod]
    fn query(query: PySelect) -> Self {
        Self(molgfx::core::RepresentationInput::Query(query.0))
    }

    #[staticmethod]
    fn source(source: String) -> Self {
        Self(molgfx::core::RepresentationInput::Source(
            source.into_boxed_str(),
        ))
    }

    #[staticmethod]
    fn volume(volume: PyVolumeHandle) -> Self {
        Self(molgfx::core::RepresentationInput::Volume(volume.0))
    }

    #[staticmethod]
    fn segmentation(segmentation: PySegmentationHandle) -> Self {
        Self(molgfx::core::RepresentationInput::Segmentation(
            segmentation.0,
        ))
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match &self.0 {
            molgfx::core::RepresentationInput::Stored(_) => "stored",
            molgfx::core::RepresentationInput::Query(_) => "query",
            molgfx::core::RepresentationInput::Source(_) => "source",
            molgfx::core::RepresentationInput::Volume(_) => "volume",
            molgfx::core::RepresentationInput::Segmentation(_) => "segmentation",
        }
    }
}

#[pyclass(name = "RepresentationParams", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRepresentationParams(pub(crate) molgfx::core::RepresentationParams);

#[pymethods]
impl PyRepresentationParams {
    #[new]
    #[pyo3(signature = (radius_scale=1.0, bond_radius=0.18, probe_radius=1.4, gaussian_sigma=1.0, isolevel=1.0, surface_kind=PySurfaceKind::SolventExcluded, surface_style=PySurfaceStyle::Solid, surface_components=None, surface_pattern_spacing=1.5, surface_pattern_width_pixels=1.25, ribbon_width=1.2, tube_radius=0.3, tube_radius_mapping=None, point_size_pixels=3.0, line_width_pixels=1.5))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        radius_scale: f32,
        bond_radius: f32,
        probe_radius: f32,
        gaussian_sigma: f32,
        isolevel: f32,
        surface_kind: PySurfaceKind,
        surface_style: PySurfaceStyle,
        surface_components: Option<PySurfaceComponentPolicy>,
        surface_pattern_spacing: f32,
        surface_pattern_width_pixels: f32,
        ribbon_width: f32,
        tube_radius: f32,
        tube_radius_mapping: Option<PyTubeRadiusMapping>,
        point_size_pixels: f32,
        line_width_pixels: f32,
    ) -> Self {
        Self(molgfx::core::RepresentationParams {
            radius_scale,
            bond_radius,
            probe_radius,
            gaussian_sigma,
            isolevel,
            surface_kind: surface_kind.into(),
            surface_style: surface_style.into(),
            surface_components: surface_components
                .map_or_else(molgfx::core::SurfaceComponentPolicy::keep_all, |policy| {
                    policy.0
                }),
            surface_pattern_spacing,
            surface_pattern_width_pixels,
            ribbon_width,
            tube_radius,
            tube_radius_mapping: tube_radius_mapping
                .map_or(molgfx::core::TubeRadiusMapping::Constant, |value| value.0),
            point_size_pixels,
            line_width_pixels,
        })
    }

    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::core::RepresentationParams::default())
    }

    #[getter]
    fn radius_scale(&self) -> f32 {
        self.0.radius_scale
    }

    #[getter]
    fn bond_radius(&self) -> f32 {
        self.0.bond_radius
    }

    #[getter]
    fn probe_radius(&self) -> f32 {
        self.0.probe_radius
    }

    #[getter]
    fn isolevel(&self) -> f32 {
        self.0.isolevel
    }

    #[getter]
    fn surface_components(&self) -> PySurfaceComponentPolicy {
        PySurfaceComponentPolicy(self.0.surface_components)
    }

    #[getter]
    fn tube_radius(&self) -> f32 {
        self.0.tube_radius
    }
}

#[pyclass(name = "RepresentationConfig", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyRepresentationConfig(pub(crate) molgfx::core::RepresentationConfig);

#[pymethods]
impl PyRepresentationConfig {
    #[new]
    fn new(kind: PyRepresentationKind) -> Self {
        Self(molgfx::core::RepresentationConfig::new(kind.into()))
    }

    #[getter]
    fn kind(&self) -> PyRepresentationKind {
        self.0.kind().into()
    }

    fn radius_scale(&self, scale: f32) -> Self {
        Self(self.0.clone().radius_scale(scale))
    }

    fn bond_radius(&self, radius: f32) -> Self {
        Self(self.0.clone().bond_radius(radius))
    }

    fn tube_radius(&self, radius: f32) -> PyResult<Self> {
        core(self.0.clone().tube_radius(radius)).map(Self)
    }

    fn putty_b_factor(&self, domain: [f32; 2], radii: [f32; 2]) -> PyResult<Self> {
        core(self.0.clone().putty_b_factor(domain, radii)).map(Self)
    }

    fn isolevel(&self, level: f32) -> Self {
        Self(self.0.clone().isolevel(level))
    }

    fn surface(&self, kind: PySurfaceKind, style: PySurfaceStyle) -> Self {
        Self(self.0.clone().surface(kind.into(), style.into()))
    }
}
