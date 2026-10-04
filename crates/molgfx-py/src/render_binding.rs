//! Rendering and semantic-picking Python adapters.

use crate::binding::error;
use crate::scene_binding::PyScene;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyDict, PyList, PyModule};

#[derive(Clone, Debug)]
#[pyclass(name = "PickResult", frozen, skip_from_py_object)]
pub(super) struct PyPickResult(pub(super) molgfx::PickResult);

#[pymethods]
impl PyPickResult {
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0.kind {
            molgfx::PickKind::Atom => "atom",
            molgfx::PickKind::Bond => "bond",
            molgfx::PickKind::Interaction => "interaction",
            molgfx::PickKind::Label => "label",
            molgfx::PickKind::Measurement => "measurement",
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

    /// Captured source scene used to reject stale picks.
    #[getter]
    const fn source_id(&self) -> Option<u64> {
        self.0.source_id
    }

    /// Captured physical segmentation identity, encoded without losing precision.
    #[getter]
    fn segmentation(&self) -> PyResult<Option<String>> {
        self.0
            .segmentation
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|cause| pyo3::exceptions::PyValueError::new_err(cause.to_string()))
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
    fn quality_json(&self) -> PyResult<String> {
        serde_json::to_string(self.0.quality())
            .map_err(|cause| crate::binding::MolgfxError::new_err(cause.to_string()))
    }

    fn pixels<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.0.pixels())
    }

    fn png_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = py.detach(|| self.0.png_bytes()).map_err(error)?;
        Ok(PyBytes::new(py, &bytes))
    }
    fn save(&self, py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<()> {
        let path = path.extract::<std::path::PathBuf>()?;
        py.detach(|| self.0.save(path)).map_err(error)
    }
}

/// A completed scene-linear half-float image; presentation effects are omitted.
#[pyclass(name = "HdrImage")]
struct PyHdrImage(molgfx::HdrImage);

#[pymethods]
impl PyHdrImage {
    #[getter]
    fn width(&self) -> u32 {
        self.0.width()
    }

    #[getter]
    fn height(&self) -> u32 {
        self.0.height()
    }

    fn quality_json(&self) -> PyResult<String> {
        serde_json::to_string(self.0.quality())
            .map_err(|cause| crate::binding::MolgfxError::new_err(cause.to_string()))
    }

    fn rgba16f<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.0.rgba16f())
    }

    fn exr_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = py.detach(|| self.0.exr_bytes()).map_err(error)?;
        Ok(PyBytes::new(py, &bytes))
    }

    /// Saves half-float `OpenEXR`; other filename extensions raise `ValueError`.
    fn save(&self, py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<()> {
        let path = path.extract::<std::path::PathBuf>()?;
        let extension = path.extension().and_then(std::ffi::OsStr::to_str);
        if !extension.is_some_and(|value| value.eq_ignore_ascii_case("exr")) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "HDR images can only be saved with an .exr extension",
            ));
        }
        py.detach(|| self.0.save_exr(path)).map_err(error)
    }
}

#[pyclass(name = "Renderer")]
struct PyRenderer(molgfx::Renderer);

#[pymethods]
impl PyRenderer {
    #[new]
    #[pyo3(signature = (*, profile=None, surface_memory_mib=None))]
    fn new(
        py: Python<'_>,
        profile: Option<&crate::authoring_binding::PyRenderProfile>,
        surface_memory_mib: Option<u64>,
    ) -> PyResult<Self> {
        let profile = profile.map_or_else(molgfx::RenderProfile::default, |profile| profile.0);
        let budget = surface_memory_mib.map(|mib| mib.saturating_mul(1024 * 1024));
        py.detach(|| molgfx::Renderer::with_surface_budget(profile, budget))
            .map(Self)
            .map_err(error)
    }

    #[pyo3(signature = (scene, *, size))]
    fn render_image(
        &mut self,
        py: Python<'_>,
        scene: &PyScene,
        size: (u32, u32),
    ) -> PyResult<PyImage> {
        py.detach(|| self.0.render_image(&scene.inner, size))
            .map(PyImage)
            .map_err(error)
    }

    /// Renders a converged, bloom-free scene-linear HDR exposure.
    #[pyo3(signature = (scene, *, size))]
    fn render_hdr_image(
        &mut self,
        py: Python<'_>,
        scene: &PyScene,
        size: (u32, u32),
    ) -> PyResult<PyHdrImage> {
        py.detach(|| self.0.render_hdr_image(&scene.inner, size))
            .map(PyHdrImage)
            .map_err(error)
    }

    fn pick(&mut self, py: Python<'_>, x: u32, y: u32) -> PyResult<Option<PyPickResult>> {
        py.detach(|| self.0.pick(x, y))
            .map(|pick| pick.map(PyPickResult))
            .map_err(error)
    }

    #[pyo3(signature = (scene, *, size, fps, frames))]
    fn render_sequence(
        &mut self,
        py: Python<'_>,
        scene: &PyScene,
        size: (u32, u32),
        fps: u32,
        frames: usize,
    ) -> PyResult<Vec<PyImage>> {
        py.detach(|| self.0.render_sequence(&scene.inner, size, fps, frames))
            .map(|images| images.into_iter().map(PyImage).collect())
            .map_err(error)
    }

    #[pyo3(signature = (scene, path, *, size, fps))]
    fn render_camera_path(
        &mut self,
        py: Python<'_>,
        scene: &PyScene,
        path: &crate::camera_path_binding::PyCameraPath,
        size: (u32, u32),
        fps: u32,
    ) -> PyResult<Vec<PyImage>> {
        py.detach(|| self.0.render_camera_path(&scene.inner, &path.0, size, fps))
            .map(|images| images.into_iter().map(PyImage).collect())
            .map_err(error)
    }

    fn explain(&self, scene: &PyScene) -> String {
        self.0.explain(&scene.inner)
    }
}

/// What the build carries and what the host offers, for diagnosing a failed
/// `Renderer()`.
#[pyfunction]
fn system_info(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let info = py.detach(molgfx::system_info);
    let adapters = PyList::empty(py);
    for adapter in &info.adapters {
        let entry = PyDict::new(py);
        entry.set_item("name", &adapter.name)?;
        entry.set_item("backend", adapter.backend)?;
        entry.set_item("type", adapter.device_type)?;
        entry.set_item(
            "max_storage_buffers_per_shader_stage",
            adapter.max_storage_buffers_per_shader_stage,
        )?;
        entry.set_item("max_texture_dim_3d", adapter.max_texture_dim_3d)?;
        entry.set_item("renderer_core_compatible", adapter.renderer_core_compatible)?;
        entry.set_item("incompatibility_reasons", &adapter.incompatibility_reasons)?;
        entry.set_item("occupancy_rg32_storage", adapter.occupancy_rg32_storage)?;
        entry.set_item("occupancy_rgba32_storage", adapter.occupancy_rgba32_storage)?;
        adapters.append(entry)?;
    }
    let report = PyDict::new(py);
    report.set_item("platform", info.platform)?;
    report.set_item("compiled_backends", info.compiled_backends)?;
    report.set_item("available_adapters", adapters)?;
    Ok(report)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(system_info, module)?)?;
    module.add_class::<PyPickResult>()?;
    module.add_class::<PyImage>()?;
    module.add_class::<PyHdrImage>()?;
    module.add_class::<PyRenderer>()
}
