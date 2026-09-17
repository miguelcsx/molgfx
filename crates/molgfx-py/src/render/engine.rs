//! Python adapters for device-backed rendering and image readback.

use crate::core::{PyScene, PyVolumeSegmentRef};
use crate::error::{render, value};
use crate::math::PyCamera;
use crate::memory::{PyMemoryOwnership, PyMemoryTransferExclusion};
use numpy::ndarray::Array3;
use numpy::{
    IntoPyArray, PyArray3, PyArrayMethods, PyReadonlyArray3, PyReadonlyArrayDyn,
    PyUntypedArrayMethods,
};
use pyo3::exceptions::PyIOError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

#[pyclass(name = "Image")]
#[derive(Debug)]
pub(crate) struct PyImage {
    image: Option<molgfx::Image>,
    width: u32,
    height: u32,
}

impl From<molgfx::Image> for PyImage {
    fn from(image: molgfx::Image) -> Self {
        Self {
            width: image.width,
            height: image.height,
            image: Some(image),
        }
    }
}

#[pymethods]
impl PyImage {
    #[staticmethod]
    fn copy_from_numpy(pixels: PyReadonlyArray3<'_, u8>) -> PyResult<Self> {
        let shape = pixels.shape();
        if shape.len() != 3 || shape[2] != 4 {
            return Err(value("pixels must have shape (height, width, 4)"));
        }
        let pixels = pixels
            .as_slice()
            .map_err(|_| value("pixels must be C-contiguous uint8"))?;
        let height = u32::try_from(shape[0]).map_err(|_| value("image height exceeds u32"))?;
        let width = u32::try_from(shape[1]).map_err(|_| value("image width exceeds u32"))?;
        Ok(molgfx::Image {
            width,
            height,
            pixels: pixels.to_vec(),
        }
        .into())
    }

    #[getter]
    fn width(&self) -> u32 {
        self.width
    }

    #[getter]
    fn height(&self) -> u32 {
        self.height
    }

    #[getter]
    fn numpy_ownership(&self) -> PyMemoryOwnership {
        PyMemoryOwnership::Transferred
    }

    #[getter]
    fn buffer_pointer(&self) -> Option<usize> {
        self.image
            .as_ref()
            .map(|image| image.pixels.as_ptr() as usize)
    }

    #[getter]
    fn transfer_deleter(&self) -> &'static str {
        "numpy"
    }

    fn transfer_numpy<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyArray3<u8>>> {
        let image = self
            .image
            .take()
            .ok_or_else(|| value("image pixels have already been moved to NumPy"))?;
        super::image_transfer::transfer_image_array(py, image)
    }

    fn copy_png_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let image = self
            .image
            .as_ref()
            .ok_or_else(|| value("image pixels have already been moved to NumPy"))?;
        let bytes = render(image.png_bytes())?;
        Ok(PyBytes::new(py, &bytes))
    }
}

#[pyclass(name = "HdrImage")]
#[derive(Debug)]
pub(crate) struct PyHdrImage {
    image: molgfx::HdrImage,
}

impl From<molgfx::HdrImage> for PyHdrImage {
    fn from(image: molgfx::HdrImage) -> Self {
        Self { image }
    }
}

#[pymethods]
impl PyHdrImage {
    #[getter]
    fn width(&self) -> u32 {
        self.image.width()
    }

    #[getter]
    fn height(&self) -> u32 {
        self.image.height()
    }

    #[getter]
    fn numpy_ownership(&self) -> PyMemoryOwnership {
        PyMemoryOwnership::Copied
    }

    #[getter]
    fn transfer_exclusion(&self) -> PyMemoryTransferExclusion {
        PyMemoryTransferExclusion::RustStorageNotMovable
    }

    fn copy_rgba16f_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray3<u8>>> {
        let height = usize::try_from(self.image.height())
            .map_err(|_| value("HDR image height exceeds Python limits"))?;
        let width = usize::try_from(self.image.width())
            .map_err(|_| value("HDR image width exceeds Python limits"))?;
        let array = Array3::from_shape_vec((height, width, 8), self.image.rgba16f().to_vec())
            .map_err(|error| value(error.to_string()))?;
        let result = array.into_pyarray(py);
        result.readwrite().make_nonwriteable();
        Ok(result)
    }

    fn write_exr(&self, py: Python<'_>, path: PathBuf) -> PyResult<()> {
        let file = File::create(&path).map_err(|error| {
            PyIOError::new_err(format!("could not create {}: {error}", path.display()))
        })?;
        let mut writer = BufWriter::with_capacity(64 * 1_024, file);
        let result = py.detach(|| self.image.write_exr(&mut writer));
        render(result)?;
        writer.flush().map_err(|error| {
            PyIOError::new_err(format!("could not flush {}: {error}", path.display()))
        })
    }
}

#[pyclass(name = "FrameTiming", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyFrameTiming(molgfx::FrameTiming);

#[pymethods]
impl PyFrameTiming {
    #[getter]
    fn gpu_ns(&self) -> u64 {
        self.0.gpu_ns
    }
    #[getter]
    fn cpu_ns(&self) -> u64 {
        self.0.cpu_ns
    }
    #[getter]
    fn frame_ns(&self) -> u64 {
        self.0.frame_ns
    }
}

#[pyclass(name = "PickEntity", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPickEntity(pub(crate) molgfx::PickEntity);

#[pymethods]
impl PyPickEntity {
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            molgfx::PickEntity::Structure(entity) => super::entity_kind::name(entity.kind()),
            molgfx::PickEntity::VolumeSegment(_) => "volume_segment",
        }
    }

    #[getter]
    fn global_identity(&self) -> Option<PyGlobalPickIdentity> {
        match self.0 {
            molgfx::PickEntity::Structure(entity) => Some(PyGlobalPickIdentity(entity)),
            molgfx::PickEntity::VolumeSegment(_) => None,
        }
    }

    #[getter]
    fn volume_segment(&self) -> Option<PyVolumeSegmentRef> {
        match self.0 {
            molgfx::PickEntity::Structure(_) => None,
            molgfx::PickEntity::VolumeSegment(segment) => Some(segment.into()),
        }
    }
}

#[pyclass(name = "GlobalPickIdentity", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyGlobalPickIdentity(molgfx::GlobalPickIdentity);

#[pymethods]
impl PyGlobalPickIdentity {
    #[getter]
    fn dataset(&self) -> u64 {
        self.0.dataset().get()
    }

    #[getter]
    fn chunk(&self) -> u64 {
        self.0.chunk().get()
    }

    #[getter]
    fn row(&self) -> u64 {
        self.0.row().get()
    }

    #[getter]
    fn kind(&self) -> &'static str {
        super::entity_kind::name(self.0.kind())
    }
}

#[pyclass(name = "Pick", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyPick {
    entity: PyPickEntity,
    selection_indices: Vec<u32>,
}

impl From<molgfx::Pick> for PyPick {
    fn from(value: molgfx::Pick) -> Self {
        let selection_indices = match value.selection {
            molgfx::AtomSelection::Sparse(indices) => indices,
            molgfx::AtomSelection::Range(range) => range.collect(),
            _ => Vec::new(),
        };
        Self {
            entity: PyPickEntity(value.entity),
            selection_indices,
        }
    }
}

#[pymethods]
impl PyPick {
    #[getter]
    fn entity(&self) -> PyPickEntity {
        self.entity
    }

    #[getter]
    fn selection_indices(&self) -> Vec<u32> {
        self.selection_indices.clone()
    }
}

#[pyclass(name = "Engine")]
#[derive(Debug)]
pub(crate) struct PyEngine {
    pub(super) inner: molgfx::Engine,
    trajectory_windows: Vec<molgfx::TrajectoryChunkWindow>,
    pub(super) residency_output: molgfx::ResidencyOutput,
    pub(super) point_placements: Vec<molgfx::PointChunkPlacement>,
    pub(super) instance_placements: Vec<molgfx::InstanceChunkPlacement>,
}

#[pymethods]
impl PyEngine {
    #[new]
    #[pyo3(signature = (config=None))]
    fn new(config: Option<super::PyEngineConfig>) -> PyResult<Self> {
        let config = config.map_or_else(molgfx::EngineConfig::default, |value| value.0);
        let trajectory_windows = Vec::with_capacity(config.residency.machine_capacity);
        let point_placements = Vec::with_capacity(config.residency.machine_capacity);
        let instance_placements = Vec::with_capacity(config.residency.machine_capacity);
        render(molgfx::Engine::new(&config, None)).map(|inner| Self {
            inner,
            trajectory_windows,
            residency_output: molgfx::ResidencyOutput::default(),
            point_placements,
            instance_placements,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.inner.resize(width, height);
    }

    fn set_render_mode(&mut self, mode: super::PyRenderMode) {
        self.inner.set_render_mode(mode.into());
    }

    fn set_render_profile(&mut self, profile: super::PyRenderProfile) -> PyResult<()> {
        render(self.inner.set_render_profile(profile.0))
    }

    fn sequence(
        &self,
        config: super::PySequenceConfig,
    ) -> PyResult<super::engine_sequence::PySequenceRenderer> {
        super::engine_sequence::create(&self.inner, config)
    }

    fn set_trajectory_chunk_windows(
        &mut self,
        windows: Vec<crate::trajectory::PyTrajectoryChunkWindow>,
    ) -> PyResult<()> {
        if windows.len() > self.trajectory_windows.capacity() {
            return Err(crate::error::value(
                "trajectory window count exceeds engine residency capacity",
            ));
        }
        self.trajectory_windows.clear();
        self.trajectory_windows
            .extend(windows.into_iter().map(|window| window.0));
        crate::error::render(
            self.inner
                .set_trajectory_chunk_windows(&self.trajectory_windows)
                .map_err(molgfx::RenderError::from),
        )
    }

    fn render(
        &mut self,
        py: Python<'_>,
        scene: &PyScene,
        camera: PyCamera,
    ) -> PyResult<super::PyFrameReport> {
        render(py.detach(|| self.inner.render(&scene.inner, &camera.inner)))
            .map(super::PyFrameReport)
    }

    fn render_image(
        &mut self,
        py: Python<'_>,
        scene: &PyScene,
        camera: PyCamera,
        width: u32,
        height: u32,
    ) -> PyResult<PyImage> {
        render(py.detach(|| {
            self.inner.render_image(
                &scene.inner,
                &camera.inner,
                molgfx::ImageConfig { width, height },
            )
        }))
        .map(Into::into)
    }

    fn render_hdr_image(
        &mut self,
        py: Python<'_>,
        scene: &PyScene,
        camera: PyCamera,
        config: super::PyImageConfig,
    ) -> PyResult<PyHdrImage> {
        render(py.detach(|| {
            self.inner
                .render_hdr_image(&scene.inner, &camera.inner, config.0)
        }))
        .map(Into::into)
    }

    fn pick(&mut self, x: u32, y: u32) -> PyResult<Option<PyPick>> {
        render(self.inner.pick(x, y)).map(|pick| pick.map(Into::into))
    }

    fn profile_frame(
        &mut self,
        py: Python<'_>,
        scene: &PyScene,
        camera: PyCamera,
        config: super::PyImageConfig,
    ) -> PyResult<PyFrameTiming> {
        render(py.detach(|| {
            self.inner
                .profile_frame(&scene.inner, &camera.inner, config.0)
        }))
        .map(PyFrameTiming)
    }

    fn install_brick_atlas(
        &mut self,
        catalog: &super::brick::PyBrickCatalog,
        config: super::brick_atlas::PyBrickAtlasConfig,
    ) -> PyResult<usize> {
        super::brick_atlas::install(&mut self.inner, catalog, config)
    }

    fn stage_brick(
        &mut self,
        atlas: usize,
        descriptor: super::brick::PyBrickDescriptor,
        bytes: PyReadonlyArrayDyn<'_, u8>,
    ) -> PyResult<super::brick_atlas::PyFenceValue> {
        super::brick_atlas::stage(&mut self.inner, atlas, descriptor, bytes)
    }

    fn evict_brick(
        &mut self,
        atlas: usize,
        brick: super::brick::PyBrickId,
    ) -> PyResult<super::brick_atlas::PyFenceValue> {
        super::brick_atlas::evict(&mut self.inner, atlas, brick)
    }

    fn brick_atlas_metrics(
        &mut self,
        atlas: usize,
    ) -> Option<super::brick_atlas::PyBrickAtlasMetrics> {
        super::brick_atlas::metrics(&mut self.inner, atlas)
    }

    #[getter]
    fn render_mode(&self) -> super::PyRenderMode {
        self.inner.render_mode().into()
    }

    #[getter]
    fn capabilities(&self) -> super::PyCapabilities {
        self.inner.capabilities().into()
    }

    #[getter]
    fn render_profile(&self) -> super::PyRenderProfile {
        super::PyRenderProfile(self.inner.render_profile().clone())
    }

    #[getter]
    fn resolved_render_plan(&self) -> super::PyResolvedRenderPlan {
        super::PyResolvedRenderPlan(*self.inner.resolved_render_plan())
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyImage>()?;
    module.add_class::<PyHdrImage>()?;
    module.add_class::<PyFrameTiming>()?;
    module.add_class::<PyPickEntity>()?;
    module.add_class::<PyGlobalPickIdentity>()?;
    module.add_class::<PyPick>()?;
    module.add_class::<PyEngine>()?;
    super::engine_sequence::register(module)
}
