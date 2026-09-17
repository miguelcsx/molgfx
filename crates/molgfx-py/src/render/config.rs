//! Python adapters for engine and image configuration.

use super::PyRenderProfile;
use pyo3::prelude::*;

#[pyclass(name = "PowerPreference", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyPowerPreference {
    HighPerformance,
    LowPower,
}

impl From<PyPowerPreference> for molgfx::PowerPreference {
    fn from(value: PyPowerPreference) -> Self {
        match value {
            PyPowerPreference::HighPerformance => Self::HighPerformance,
            PyPowerPreference::LowPower => Self::LowPower,
        }
    }
}

impl From<molgfx::PowerPreference> for PyPowerPreference {
    fn from(value: molgfx::PowerPreference) -> Self {
        match value {
            molgfx::PowerPreference::HighPerformance => Self::HighPerformance,
            molgfx::PowerPreference::LowPower => Self::LowPower,
        }
    }
}

#[pyclass(name = "RenderMode", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyRenderMode {
    Realtime,
    Cinematic,
}

impl From<PyRenderMode> for molgfx::RenderMode {
    fn from(value: PyRenderMode) -> Self {
        match value {
            PyRenderMode::Realtime => Self::Realtime,
            PyRenderMode::Cinematic => Self::Cinematic,
        }
    }
}

impl From<molgfx::RenderMode> for PyRenderMode {
    fn from(value: molgfx::RenderMode) -> Self {
        match value {
            molgfx::RenderMode::Realtime => Self::Realtime,
            molgfx::RenderMode::Cinematic => Self::Cinematic,
        }
    }
}

#[pyclass(name = "FrameStatus", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyFrameStatus {
    Presented,
    Skipped,
}

impl From<molgfx::FrameStatus> for PyFrameStatus {
    fn from(value: molgfx::FrameStatus) -> Self {
        match value {
            molgfx::FrameStatus::Presented => Self::Presented,
            molgfx::FrameStatus::Skipped => Self::Skipped,
        }
    }
}

#[pyclass(name = "FrameReport", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFrameReport(pub(crate) molgfx::FrameReport);

#[pymethods]
impl PyFrameReport {
    #[getter]
    fn status(&self) -> PyFrameStatus {
        self.0.status.into()
    }

    #[getter]
    fn complete(&self) -> bool {
        self.0.completeness == molgfx::FrameCompleteness::Complete
    }

    #[getter]
    fn pending_chunks(&self) -> u64 {
        match self.0.completeness {
            molgfx::FrameCompleteness::Complete => 0,
            molgfx::FrameCompleteness::Progressive { pending_chunks } => pending_chunks,
        }
    }

    #[getter]
    fn streaming_proxy(&self) -> bool {
        self.0
            .degradation
            .contains(molgfx::FrameDegradation::STREAMING_PROXY)
    }

    #[getter]
    fn needs_another_frame(&self) -> bool {
        self.0.needs_another_frame
    }

    #[getter]
    fn tracked_chunks(&self) -> usize {
        self.0.metrics.tracked_chunks
    }

    #[getter]
    fn upload_in_flight_bytes(&self) -> u64 {
        self.0.metrics.upload_in_flight_bytes
    }

    #[getter]
    fn derived_cache_gpu_bytes(&self) -> u64 {
        self.0.metrics.derived_cache_gpu_bytes
    }

    #[getter]
    fn derived_cache_peak_gpu_bytes(&self) -> u64 {
        self.0.metrics.derived_cache_peak_gpu_bytes
    }
}

#[pyclass(name = "DerivedCacheBudget", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDerivedCacheBudget(pub(crate) molgfx::DerivedCacheBudget);

#[pymethods]
impl PyDerivedCacheBudget {
    #[new]
    #[pyo3(signature = (cpu_bytes, gpu_bytes))]
    fn new(cpu_bytes: u64, gpu_bytes: u64) -> Self {
        Self(molgfx::DerivedCacheBudget {
            cpu_bytes,
            gpu_bytes,
        })
    }

    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::DerivedCacheBudget::default())
    }

    #[getter]
    fn cpu_bytes(&self) -> u64 {
        self.0.cpu_bytes
    }

    #[getter]
    fn gpu_bytes(&self) -> u64 {
        self.0.gpu_bytes
    }
}

#[pyclass(name = "ImageConfig", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyImageConfig(pub(crate) molgfx::ImageConfig);

#[pymethods]
impl PyImageConfig {
    #[new]
    fn new(width: u32, height: u32) -> Self {
        Self(molgfx::ImageConfig { width, height })
    }
    #[staticmethod]
    fn publication_4k() -> Self {
        Self(molgfx::ImageConfig::publication_4k())
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

#[pyclass(name = "SequenceConfig", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PySequenceConfig(pub(crate) molgfx::SequenceConfig);

#[pymethods]
impl PySequenceConfig {
    #[new]
    #[pyo3(signature = (width, height, frames_per_second, max_in_flight=3))]
    fn new(width: u32, height: u32, frames_per_second: u32, max_in_flight: u8) -> PyResult<Self> {
        crate::error::render(molgfx::SequenceConfig::at_fps(
            molgfx::ImageConfig { width, height },
            frames_per_second,
            max_in_flight,
        ))
        .map(Self)
    }

    #[getter]
    fn width(&self) -> u32 {
        self.0.image.width
    }

    #[getter]
    fn height(&self) -> u32 {
        self.0.image.height
    }

    #[getter]
    fn timebase_nanoseconds(&self) -> u64 {
        self.0.timebase_nanoseconds
    }

    #[getter]
    fn max_in_flight(&self) -> u8 {
        self.0.max_in_flight
    }
}

#[pyclass(name = "FrameTicket", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFrameTicket(pub(crate) molgfx::FrameTicket);

#[pymethods]
impl PyFrameTicket {
    #[getter]
    fn index(&self) -> u64 {
        self.0.index
    }

    #[getter]
    fn timestamp(&self) -> u64 {
        self.0.timestamp
    }
}

#[pyclass(name = "EngineConfig", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyEngineConfig(pub(crate) molgfx::EngineConfig);

#[pymethods]
impl PyEngineConfig {
    #[new]
    #[pyo3(signature = (width=1280, height=800, mode=None, profile=None, power=None, derived_cache=None))]
    fn new(
        width: u32,
        height: u32,
        mode: Option<PyRenderMode>,
        profile: Option<PyRenderProfile>,
        power: Option<PyPowerPreference>,
        derived_cache: Option<PyDerivedCacheBudget>,
    ) -> Self {
        let mut value = molgfx::EngineConfig {
            width,
            height,
            ..molgfx::EngineConfig::default()
        };
        if let Some(mode) = mode {
            value.mode = mode.into();
        }
        if let Some(profile) = profile {
            value.profile = profile.0;
        }
        if let Some(power) = power {
            value.power = power.into();
        }
        if let Some(derived_cache) = derived_cache {
            value.derived_cache = derived_cache.0;
        }
        Self(value)
    }
    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::EngineConfig::default())
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
    #[getter]
    fn power(&self) -> PyPowerPreference {
        self.0.power.into()
    }
    #[getter]
    fn derived_cache(&self) -> PyDerivedCacheBudget {
        PyDerivedCacheBudget(self.0.derived_cache)
    }
}

#[pyclass(name = "Capabilities", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyCapabilities {
    max_storage_buffer_bytes: u64,
    max_texture_dim: u32,
    max_texture_dim_3d: u32,
    feature_flags: u8,
}

impl PyCapabilities {
    const HARDWARE_RAY_TRACING: u8 = 1 << 0;
    const MESH_SHADERS: u8 = 1 << 1;
    const BINDLESS: u8 = 1 << 2;
    const TIMESTAMP_QUERIES: u8 = 1 << 3;
    const SUBGROUP_OPS: u8 = 1 << 4;

    const fn has_feature(self, flag: u8) -> bool {
        self.feature_flags & flag != 0
    }
}

impl From<&molgfx::Capabilities> for PyCapabilities {
    fn from(value: &molgfx::Capabilities) -> Self {
        Self {
            max_storage_buffer_bytes: value.max_storage_buffer_bytes,
            max_texture_dim: value.max_texture_dim,
            max_texture_dim_3d: value.max_texture_dim_3d,
            feature_flags: (u8::from(value.hardware_ray_tracing()) * Self::HARDWARE_RAY_TRACING)
                | (u8::from(value.mesh_shaders()) * Self::MESH_SHADERS)
                | (u8::from(value.bindless()) * Self::BINDLESS)
                | (u8::from(value.timestamp_queries()) * Self::TIMESTAMP_QUERIES)
                | (u8::from(value.subgroup_ops()) * Self::SUBGROUP_OPS),
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
        self.has_feature(Self::HARDWARE_RAY_TRACING)
    }
    #[getter]
    fn mesh_shaders(&self) -> bool {
        self.has_feature(Self::MESH_SHADERS)
    }
    #[getter]
    fn bindless(&self) -> bool {
        self.has_feature(Self::BINDLESS)
    }
    #[getter]
    fn timestamp_queries(&self) -> bool {
        self.has_feature(Self::TIMESTAMP_QUERIES)
    }
    #[getter]
    fn subgroup_ops(&self) -> bool {
        self.has_feature(Self::SUBGROUP_OPS)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyPowerPreference>()?;
    module.add_class::<PyRenderMode>()?;
    module.add_class::<PyFrameStatus>()?;
    module.add_class::<PyFrameReport>()?;
    module.add_class::<PyDerivedCacheBudget>()?;
    module.add_class::<PyImageConfig>()?;
    module.add_class::<PySequenceConfig>()?;
    module.add_class::<PyFrameTicket>()?;
    module.add_class::<PyEngineConfig>()?;
    module.add_class::<PyCapabilities>()
}
