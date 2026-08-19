//! Python adapters for engine and image configuration.

use super::PyRenderProfile;
use pyo3::prelude::*;

#[pyclass(name = "RenderMode", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyRenderMode {
    Realtime,
    Quality,
}

impl From<PyRenderMode> for pdviewx::RenderMode {
    fn from(value: PyRenderMode) -> Self {
        match value {
            PyRenderMode::Realtime => Self::Realtime,
            PyRenderMode::Quality => Self::Quality,
        }
    }
}

impl From<pdviewx::RenderMode> for PyRenderMode {
    fn from(value: pdviewx::RenderMode) -> Self {
        match value {
            pdviewx::RenderMode::Realtime => Self::Realtime,
            pdviewx::RenderMode::Quality => Self::Quality,
        }
    }
}

#[pyclass(name = "FrameOutcome", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyFrameOutcome {
    Presented,
    Skipped,
}

impl From<pdviewx::FrameOutcome> for PyFrameOutcome {
    fn from(value: pdviewx::FrameOutcome) -> Self {
        match value {
            pdviewx::FrameOutcome::Presented => Self::Presented,
            pdviewx::FrameOutcome::Skipped => Self::Skipped,
        }
    }
}

#[pyclass(name = "ImageConfig", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyImageConfig(pub(crate) pdviewx::ImageConfig);

#[pymethods]
impl PyImageConfig {
    #[new]
    fn new(width: u32, height: u32) -> Self {
        Self(pdviewx::ImageConfig { width, height })
    }
    #[staticmethod]
    fn publication_4k() -> Self {
        Self(pdviewx::ImageConfig::publication_4k())
    }
    #[getter]
    fn width(&self) -> u32 {
        self.0.width
    }
    #[getter]
    fn height(&self) -> u32 {
        self.0.height
    }
}

#[pyclass(name = "EngineConfig", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyEngineConfig(pub(crate) pdviewx::EngineConfig);

#[pymethods]
impl PyEngineConfig {
    #[new]
    #[pyo3(signature = (width=1280, height=800, mode=None, profile=None))]
    fn new(
        width: u32,
        height: u32,
        mode: Option<PyRenderMode>,
        profile: Option<PyRenderProfile>,
    ) -> Self {
        let mut value = pdviewx::EngineConfig {
            width,
            height,
            ..pdviewx::EngineConfig::default()
        };
        if let Some(mode) = mode {
            value.mode = mode.into();
        }
        if let Some(profile) = profile {
            value.profile = profile.0;
        }
        Self(value)
    }
    #[staticmethod]
    fn default() -> Self {
        Self(pdviewx::EngineConfig::default())
    }
    #[getter]
    fn width(&self) -> u32 {
        self.0.width
    }
    #[getter]
    fn height(&self) -> u32 {
        self.0.height
    }
    #[getter]
    fn mode(&self) -> PyRenderMode {
        self.0.mode.into()
    }
    #[getter]
    fn profile(&self) -> PyRenderProfile {
        PyRenderProfile(self.0.profile.clone())
    }
}

#[pyclass(name = "Capabilities", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct PyCapabilities {
    max_storage_buffer_bytes: u64,
    max_texture_dim: u32,
    max_texture_dim_3d: u32,
    hardware_ray_tracing: bool,
    mesh_shaders: bool,
    bindless: bool,
    timestamp_queries: bool,
    subgroup_ops: bool,
}

impl From<&pdviewx::Capabilities> for PyCapabilities {
    fn from(value: &pdviewx::Capabilities) -> Self {
        Self {
            max_storage_buffer_bytes: value.max_storage_buffer_bytes,
            max_texture_dim: value.max_texture_dim,
            max_texture_dim_3d: value.max_texture_dim_3d,
            hardware_ray_tracing: value.hardware_ray_tracing(),
            mesh_shaders: value.mesh_shaders(),
            bindless: value.bindless(),
            timestamp_queries: value.timestamp_queries(),
            subgroup_ops: value.subgroup_ops(),
        }
    }
}

#[pymethods]
impl PyCapabilities {
    #[getter]
    fn max_storage_buffer_bytes(&self) -> u64 {
        self.max_storage_buffer_bytes
    }
    #[getter]
    fn max_texture_dim(&self) -> u32 {
        self.max_texture_dim
    }
    #[getter]
    fn max_texture_dim_3d(&self) -> u32 {
        self.max_texture_dim_3d
    }
    #[getter]
    fn hardware_ray_tracing(&self) -> bool {
        self.hardware_ray_tracing
    }
    #[getter]
    fn mesh_shaders(&self) -> bool {
        self.mesh_shaders
    }
    #[getter]
    fn bindless(&self) -> bool {
        self.bindless
    }
    #[getter]
    fn timestamp_queries(&self) -> bool {
        self.timestamp_queries
    }
    #[getter]
    fn subgroup_ops(&self) -> bool {
        self.subgroup_ops
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRenderMode>()?;
    module.add_class::<PyFrameOutcome>()?;
    module.add_class::<PyImageConfig>()?;
    module.add_class::<PyEngineConfig>()?;
    module.add_class::<PyCapabilities>()
}
