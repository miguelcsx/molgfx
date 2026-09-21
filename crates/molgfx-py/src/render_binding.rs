//! Rendering and semantic-picking Python adapters.

use crate::binding::{PyScene, error};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyModule};

#[derive(Clone, Debug)]
#[pyclass(name = "PickResult", frozen, skip_from_py_object)]
struct PyPickResult(molgfx::PickResult);

#[pymethods]
impl PyPickResult {
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0.kind {
            molgfx::PickKind::Atom => "atom",
            molgfx::PickKind::Bond => "bond",
            molgfx::PickKind::Interaction => "interaction",
            molgfx::PickKind::Label => "label",
            molgfx::PickKind::Primitive => "primitive",
            molgfx::PickKind::Mesh => "mesh",
            molgfx::PickKind::LigandPoseBatch => "ligand_pose_batch",
            molgfx::PickKind::Guide => "guide",
            molgfx::PickKind::DynamicBond => "dynamic_bond",
            molgfx::PickKind::Point => "point",
            molgfx::PickKind::Instance => "instance",
            molgfx::PickKind::TemplatePart => "template_part",
            molgfx::PickKind::Relation => "relation",
            molgfx::PickKind::VolumeSegment => "volume_segment",
        }
    }

    #[getter]
    const fn dataset(&self) -> Option<u64> {
        self.0.dataset
    }

    #[getter]
    const fn chunk(&self) -> Option<u64> {
        self.0.chunk
    }

    #[getter]
    const fn row(&self) -> Option<u64> {
        self.0.row
    }

    #[getter]
    const fn volume_label(&self) -> Option<u32> {
        self.0.volume_label
    }
}

#[pyclass(name = "Image")]
struct PyImage(molgfx::Image);

#[pymethods]
impl PyImage {
    #[getter]
    fn width(&self) -> u32 {
        self.0.width()
    }
    #[getter]
    fn height(&self) -> u32 {
        self.0.height()
    }
    fn png_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        self.0
            .png_bytes()
            .map(|bytes| PyBytes::new(py, &bytes))
            .map_err(error)
    }
    fn save(&self, path: &str) -> PyResult<()> {
        self.0.save(path).map_err(error)
    }
}

#[pyclass(name = "Renderer")]
struct PyRenderer(molgfx::Renderer);

#[pymethods]
impl PyRenderer {
    #[new]
    #[pyo3(signature = (*, profile=None))]
    fn new(profile: Option<&crate::authoring_binding::PyRenderProfile>) -> PyResult<Self> {
        match profile {
            Some(profile) => molgfx::Renderer::with_profile(profile.0),
            None => molgfx::Renderer::new(),
        }
        .map(Self)
        .map_err(error)
    }

    #[pyo3(signature = (scene, *, size))]
    fn render_image(&mut self, scene: &PyScene, size: (u32, u32)) -> PyResult<PyImage> {
        self.0
            .render_image(&scene.inner, size)
            .map(PyImage)
            .map_err(error)
    }

    fn pick(&mut self, x: u32, y: u32) -> PyResult<Option<PyPickResult>> {
        self.0
            .pick(x, y)
            .map(|pick| pick.map(PyPickResult))
            .map_err(error)
    }

    fn explain(&self, scene: &PyScene) -> String {
        self.0.explain(&scene.inner)
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyPickResult>()?;
    module.add_class::<PyImage>()?;
    module.add_class::<PyRenderer>()
}
