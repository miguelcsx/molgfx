//! Python adapters for device-backed rendering and image readback.

use crate::core::{PyEntityRef, PyScene, PyVolumeSegmentRef};
use crate::error::{render, value};
use crate::math::PyCamera;
use numpy::ndarray::Array3;
use numpy::{IntoPyArray, PyArray3, PyArrayMethods};
use pyo3::prelude::*;

#[pyclass(name = "Image")]
#[derive(Debug)]
pub(crate) struct PyImage {
    image: Option<pdviewx::Image>,
    width: u32,
    height: u32,
}

impl From<pdviewx::Image> for PyImage {
    fn from(image: pdviewx::Image) -> Self {
        Self {
            width: image.width,
            height: image.height,
            image: Some(image),
        }
    }
}

#[pymethods]
impl PyImage {
    #[getter]
    fn width(&self) -> u32 {
        self.width
    }

    #[getter]
    fn height(&self) -> u32 {
        self.height
    }

    fn numpy<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyArray3<u8>>> {
        let image = self
            .image
            .take()
            .ok_or_else(|| value("image pixels have already been moved to NumPy"))?;
        image_array(py, image)
    }

    fn png_bytes(&self) -> PyResult<Vec<u8>> {
        let image = self
            .image
            .as_ref()
            .ok_or_else(|| value("image pixels have already been moved to NumPy"))?;
        render(image.png_bytes())
    }
}

#[pyclass(name = "PickEntity", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPickEntity(pub(crate) pdviewx::PickEntity);

#[pymethods]
impl PyPickEntity {
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            pdviewx::PickEntity::Structure(entity) => match entity.kind {
                pdviewx::EntityKind::Atom => "atom",
                pdviewx::EntityKind::Bond => "bond",
                pdviewx::EntityKind::Edge => "edge",
                pdviewx::EntityKind::Label => "label",
                pdviewx::EntityKind::Primitive => "primitive",
                pdviewx::EntityKind::Mesh => "mesh",
            },
            pdviewx::PickEntity::VolumeSegment(_) => "volume_segment",
        }
    }

    #[getter]
    fn structure(&self) -> Option<PyEntityRef> {
        match self.0 {
            pdviewx::PickEntity::Structure(entity) => Some(entity.into()),
            pdviewx::PickEntity::VolumeSegment(_) => None,
        }
    }

    #[getter]
    fn volume_segment(&self) -> Option<PyVolumeSegmentRef> {
        match self.0 {
            pdviewx::PickEntity::Structure(_) => None,
            pdviewx::PickEntity::VolumeSegment(segment) => Some(segment.into()),
        }
    }
}

#[pyclass(name = "Pick", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyPick {
    entity: PyPickEntity,
    selection_indices: Vec<u32>,
}

impl From<pdviewx::Pick> for PyPick {
    fn from(value: pdviewx::Pick) -> Self {
        let selection_indices = match value.selection {
            pdviewx::AtomSelection::Sparse(indices) => indices,
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
    inner: pdviewx::Engine,
}

#[pymethods]
impl PyEngine {
    #[new]
    #[pyo3(signature = (config=None))]
    fn new(config: Option<super::PyEngineConfig>) -> PyResult<Self> {
        let config = config.map_or_else(pdviewx::EngineConfig::default, |value| value.0);
        render(pdviewx::Engine::new(&config, None)).map(|inner| Self { inner })
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

    fn render(&mut self, scene: &PyScene, camera: PyCamera) -> PyResult<super::PyFrameOutcome> {
        render(self.inner.render(&scene.inner, &camera.inner)).map(Into::into)
    }

    fn render_image<'py>(
        &mut self,
        py: Python<'py>,
        scene: &PyScene,
        camera: PyCamera,
        width: u32,
        height: u32,
    ) -> PyResult<Bound<'py, PyArray3<u8>>> {
        let image = render(self.inner.render_image(
            &scene.inner,
            &camera.inner,
            pdviewx::ImageConfig { width, height },
        ))?;
        image_array(py, image)
    }

    fn render_image_object(
        &mut self,
        scene: &PyScene,
        camera: PyCamera,
        config: super::PyImageConfig,
    ) -> PyResult<PyImage> {
        render(
            self.inner
                .render_image(&scene.inner, &camera.inner, config.0),
        )
        .map(Into::into)
    }

    fn pick(&mut self, x: u32, y: u32) -> PyResult<Option<PyPick>> {
        render(self.inner.pick(x, y)).map(|pick| pick.map(Into::into))
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

fn image_array(py: Python<'_>, image: pdviewx::Image) -> PyResult<Bound<'_, PyArray3<u8>>> {
    let height =
        usize::try_from(image.height).map_err(|_| value("image height exceeds Python limits"))?;
    let width =
        usize::try_from(image.width).map_err(|_| value("image width exceeds Python limits"))?;
    let array = Array3::from_shape_vec((height, width, 4), image.pixels)
        .map_err(|error| value(error.to_string()))?;
    let result = array.into_pyarray(py);
    let _readonly = result.readwrite().make_nonwriteable();
    Ok(result)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyImage>()?;
    module.add_class::<PyPickEntity>()?;
    module.add_class::<PyPick>()?;
    module.add_class::<PyEngine>()
}
