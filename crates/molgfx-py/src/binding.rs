//! The intentionally small Python surface.
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes, PyModule};

create_exception!(_engine, MolgfxError, PyException);
create_exception!(_engine, SpecError, MolgfxError);
create_exception!(_engine, RevisionConflict, MolgfxError);

pub(super) fn error(value: molgfx::Error) -> PyErr {
    let conflict = matches!(
        &value,
        molgfx::Error::Patch(molgfx::PatchError::RevisionConflict { .. })
    );
    let specification = matches!(
        &value,
        molgfx::Error::InvalidSpec(_) | molgfx::Error::Patch(_)
    );
    let message = value.to_string();
    drop(value);
    if conflict {
        RevisionConflict::new_err(message)
    } else if specification {
        SpecError::new_err(message)
    } else {
        MolgfxError::new_err(message)
    }
}

pub(super) fn selection(object: &Bound<'_, PyAny>) -> PyResult<String> {
    object
        .extract::<String>()
        .or_else(|_| object.getattr("source")?.extract::<String>())
        .map_err(|_| PyValueError::new_err("target must be a query string or molframe Query"))
}

#[derive(Clone, Debug)]
enum NativeRepresentation {
    Cartoon(molgfx::rep::Cartoon),
    Atom(molgfx::rep::AtomRepresentation),
    Point(molgfx::rep::PointRepresentation),
    Surface(molgfx::rep::Surface),
}

#[derive(Clone, Debug)]
#[pyclass(name = "Representation", frozen, skip_from_py_object)]
struct PyRepresentation(NativeRepresentation);

impl PyRepresentation {
    fn add_to(&self, scene: &mut molgfx::Scene) -> Result<molgfx::RepresentationId, molgfx::Error> {
        match &self.0 {
            NativeRepresentation::Cartoon(value) => scene.add(value.clone()),
            NativeRepresentation::Atom(value) => scene.add(value.clone()),
            NativeRepresentation::Point(value) => scene.add(value.clone()),
            NativeRepresentation::Surface(value) => scene.add(value.clone()),
        }
    }

    fn with_visual(&self, visual: molgfx::VisualStyle) -> Self {
        Self(match &self.0 {
            NativeRepresentation::Cartoon(value) => {
                NativeRepresentation::Cartoon(value.clone().visual(visual))
            }
            NativeRepresentation::Atom(value) => {
                NativeRepresentation::Atom(value.clone().visual(visual))
            }
            NativeRepresentation::Point(value) => {
                NativeRepresentation::Point(value.clone().visual(visual))
            }
            NativeRepresentation::Surface(value) => {
                NativeRepresentation::Surface(value.clone().visual(visual))
            }
        })
    }
}

#[pymethods]
impl PyRepresentation {
    fn explain(&self) -> String {
        match &self.0 {
            NativeRepresentation::Cartoon(value) => value.explain(),
            NativeRepresentation::Atom(value) => value.explain(),
            NativeRepresentation::Point(value) => value.explain(),
            NativeRepresentation::Surface(value) => value.explain(),
        }
    }

    fn visual(&self, style: &crate::visual_binding::PyVisualStyle) -> Self {
        self.with_visual(style.0.clone())
    }
}

#[derive(Clone, Debug)]
#[pyclass(name = "SceneSpec", frozen, skip_from_py_object)]
struct PySceneSpec(molgfx::SceneSpec);

#[pymethods]
impl PySceneSpec {
    #[new]
    fn new(source: &str) -> PyResult<Self> {
        molgfx::SceneSpec::from_json(source)
            .map(Self)
            .map_err(error)
    }

    #[getter]
    fn revision(&self) -> u64 {
        self.0.revision
    }

    fn to_json(&self) -> PyResult<String> {
        self.0.to_json().map_err(error)
    }

    fn stable_hash(&self) -> String {
        self.0.stable_hash()
    }

    fn patched(&self, patch: &PyScenePatch) -> PyResult<Self> {
        self.0.patched(&patch.0).map(Self).map_err(error)
    }
}

#[derive(Clone, Debug)]
#[pyclass(name = "ScenePatch", frozen, skip_from_py_object)]
struct PyScenePatch(molgfx::ScenePatch);

#[pymethods]
impl PyScenePatch {
    #[new]
    fn new(source: &str) -> PyResult<Self> {
        molgfx::ScenePatch::from_json(source)
            .map(Self)
            .map_err(error)
    }

    #[getter]
    fn base_revision(&self) -> u64 {
        self.0.base_revision
    }

    fn to_json(&self) -> PyResult<String> {
        self.0.to_json().map_err(error)
    }

    fn inverse(&self, base: &PySceneSpec) -> PyResult<Self> {
        self.0.inverse(&base.0).map(Self).map_err(error)
    }
}

fn color_spec(color: Option<&Bound<'_, PyAny>>) -> PyResult<Option<molgfx::ColorSpec>> {
    let Some(color) = color else {
        return Ok(None);
    };
    if let Ok(spec) = color.extract::<PyRef<'_, crate::authoring_binding::PyColorSpec>>() {
        return Ok(Some(spec.0.clone()));
    }
    color
        .extract::<(u8, u8, u8)>()
        .map(|(red, green, blue)| {
            Some(molgfx::color::uniform(molgfx::Color::rgb(red, green, blue)))
        })
        .map_err(|_| PyValueError::new_err("color must be a ColorSpec or RGB tuple"))
}

#[pyfunction]
#[pyo3(signature = (*, target, style="ribbon", width=None, opacity=1.0, color=None))]
fn cartoon(
    target: &Bound<'_, PyAny>,
    style: &str,
    width: Option<f32>,
    opacity: f32,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRepresentation> {
    let mut value = molgfx::rep::cartoon(selection(target)?).opacity(opacity);
    value = value.style(match style {
        "ribbon" => molgfx::rep::CartoonStyle::Ribbon,
        "rocket" => molgfx::rep::CartoonStyle::Rocket,
        "nucleic_acid" => molgfx::rep::CartoonStyle::NucleicAcid,
        "glycan" => molgfx::rep::CartoonStyle::Glycan,
        _ => return Err(PyValueError::new_err("unknown cartoon style")),
    });
    if let Some(width) = width {
        value = value.width(width);
    }
    if let Some(color) = color_spec(color)? {
        value = value.color(color);
    }
    Ok(PyRepresentation(NativeRepresentation::Cartoon(value)))
}

fn atom_representation(
    target: &Bound<'_, PyAny>,
    kind: &str,
    radius: Option<f32>,
    bond_radius: Option<f32>,
    opacity: f32,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRepresentation> {
    let target = selection(target)?;
    let mut value = match kind {
        "ball_and_stick" => molgfx::rep::ball_and_stick(target),
        "spacefill" => molgfx::rep::spacefill(target),
        "licorice" => molgfx::rep::licorice(target),
        "lines" => molgfx::rep::lines(target),
        _ => return Err(PyValueError::new_err("unknown atom representation")),
    }
    .opacity(opacity);
    if let Some(radius) = radius {
        value = value.radius(radius);
    }
    if let Some(radius) = bond_radius {
        value = value.bond_radius(radius);
    }
    if let Some(color) = color_spec(color)? {
        value = value.color(color);
    }
    Ok(PyRepresentation(NativeRepresentation::Atom(value)))
}

macro_rules! atom_constructor {
    ($name:ident, $kind:literal) => {
        #[pyfunction]
        #[pyo3(signature = (*, target, radius=None, bond_radius=None, opacity=1.0, color=None))]
        fn $name(
            target: &Bound<'_, PyAny>,
            radius: Option<f32>,
            bond_radius: Option<f32>,
            opacity: f32,
            color: Option<&Bound<'_, PyAny>>,
        ) -> PyResult<PyRepresentation> {
            atom_representation(target, $kind, radius, bond_radius, opacity, color)
        }
    };
}

atom_constructor!(ball_and_stick, "ball_and_stick");
atom_constructor!(spacefill, "spacefill");
atom_constructor!(licorice, "licorice");
atom_constructor!(lines, "lines");

#[pyfunction]
#[pyo3(signature = (*, target, size=None, opacity=1.0, color=None))]
fn points(
    target: &Bound<'_, PyAny>,
    size: Option<f32>,
    opacity: f32,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRepresentation> {
    let mut value = molgfx::rep::points(selection(target)?).opacity(opacity);
    if let Some(size) = size {
        value = value.size(size);
    }
    if let Some(color) = color_spec(color)? {
        value = value.color(color);
    }
    Ok(PyRepresentation(NativeRepresentation::Point(value)))
}

#[pyfunction]
#[pyo3(signature = (*, target, probe_radius=None, isolevel=None, opacity=1.0, color=None))]
fn surface(
    target: &Bound<'_, PyAny>,
    probe_radius: Option<f32>,
    isolevel: Option<f32>,
    opacity: f32,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRepresentation> {
    let mut value = molgfx::rep::surface(selection(target)?).opacity(opacity);
    if let Some(radius) = probe_radius {
        value = value.probe_radius(radius);
    }
    if let Some(level) = isolevel {
        value = value.isolevel(level);
    }
    if let Some(color) = color_spec(color)? {
        value = value.color(color);
    }
    Ok(PyRepresentation(NativeRepresentation::Surface(value)))
}

#[pyfunction]
#[pyo3(signature = (*, target, width=None, opacity=1.0, color=None))]
fn nucleic_acid(
    target: &Bound<'_, PyAny>,
    width: Option<f32>,
    opacity: f32,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRepresentation> {
    let mut value = molgfx::rep::nucleic_acid(selection(target)?).opacity(opacity);
    if let Some(width) = width {
        value = value.width(width);
    }
    if let Some(color) = color_spec(color)? {
        value = value.color(color);
    }
    Ok(PyRepresentation(NativeRepresentation::Cartoon(value)))
}

fn base_representation(
    target: &Bound<'_, PyAny>,
    paired: bool,
    opacity: f32,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRepresentation> {
    let target = selection(target)?;
    let mut value = if paired {
        molgfx::rep::base_pairs(target)
    } else {
        molgfx::rep::bases(target)
    }
    .opacity(opacity);
    if let Some(color) = color_spec(color)? {
        value = value.color(color);
    }
    Ok(PyRepresentation(NativeRepresentation::Atom(value)))
}

#[pyfunction]
#[pyo3(signature = (*, target, opacity=1.0, color=None))]
fn bases(
    target: &Bound<'_, PyAny>,
    opacity: f32,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRepresentation> {
    base_representation(target, false, opacity, color)
}

#[pyfunction]
#[pyo3(signature = (*, target, opacity=1.0, color=None))]
fn base_pairs(
    target: &Bound<'_, PyAny>,
    opacity: f32,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRepresentation> {
    base_representation(target, true, opacity, color)
}

#[pyfunction]
#[pyo3(signature = (*, target, width=None, opacity=1.0, color=None))]
fn glycan(
    target: &Bound<'_, PyAny>,
    width: Option<f32>,
    opacity: f32,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRepresentation> {
    let mut value = molgfx::rep::glycan(selection(target)?).opacity(opacity);
    if let Some(width) = width {
        value = value.width(width);
    }
    if let Some(color) = color_spec(color)? {
        value = value.color(color);
    }
    Ok(PyRepresentation(NativeRepresentation::Cartoon(value)))
}

#[pyclass(name = "Scene")]
pub(super) struct PyScene {
    pub(super) inner: molgfx::Scene,
    pub(super) pending: Option<Vec<molgfx::PatchOperation>>,
    browser_sources: Vec<(u64, String, Vec<u8>)>,
}

impl PyScene {
    pub(super) fn stage_or_apply(&mut self, operation: molgfx::PatchOperation) -> PyResult<()> {
        if let Some(pending) = &mut self.pending {
            pending.push(operation);
            return Ok(());
        }
        self.inner
            .apply(&molgfx::ScenePatch {
                base_revision: self.inner.revision(),
                operations: vec![operation],
            })
            .map_err(error)
    }
}

#[pymethods]
impl PyScene {
    #[new]
    fn new(structure: &Bound<'_, PyAny>) -> PyResult<Self> {
        let structure = molframe_py::structure_from_python(structure)?;
        let bytes = molgfx::molframe::write_bcif(&structure)
            .map_err(|findings| PyValueError::new_err(format!("{findings:?}")))?;
        let inner = molgfx::Scene::from_structure(&structure).map_err(error)?;
        Ok(Self {
            inner,
            pending: None,
            browser_sources: vec![(1, "structure.bcif".to_owned(), bytes)],
        })
    }

    fn add(&mut self, representation: &PyRepresentation) -> PyResult<u64> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "representations cannot be added inside a scene transaction",
            ));
        }
        representation
            .add_to(&mut self.inner)
            .map(molgfx::RepresentationId::get)
            .map_err(error)
    }

    fn set_visible(&mut self, representation: u64, visible: bool) -> PyResult<()> {
        self.stage_or_apply(molgfx::PatchOperation::SetVisibility {
            id: molgfx::RepresentationId::new(representation),
            visible,
        })
    }

    fn set_opacity(&mut self, representation: u64, opacity: f32) -> PyResult<()> {
        self.stage_or_apply(molgfx::PatchOperation::SetOpacity {
            id: molgfx::RepresentationId::new(representation),
            opacity,
        })
    }

    fn set_visual(
        &mut self,
        representation: u64,
        visual: Option<&crate::visual_binding::PyVisualStyle>,
    ) -> PyResult<()> {
        self.stage_or_apply(molgfx::PatchOperation::SetVisual {
            id: molgfx::RepresentationId::new(representation),
            visual: visual.map(|value| value.0.clone()),
        })
    }

    fn focus(&mut self, target: &Bound<'_, PyAny>) -> PyResult<()> {
        self.stage_or_apply(molgfx::PatchOperation::SetFocus {
            selection: Some(selection(target)?.into()),
        })
    }

    fn apply(&mut self, patch: &PyScenePatch) -> PyResult<()> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "patches cannot be applied inside a scene transaction",
            ));
        }
        self.inner.apply(&patch.0).map_err(error)
    }

    #[getter]
    fn revision(&self) -> u64 {
        self.inner.revision()
    }

    fn to_json(&self) -> PyResult<String> {
        self.inner.to_spec().to_json().map_err(error)
    }
    #[getter]
    fn spec(&self) -> PySceneSpec {
        PySceneSpec(self.inner.to_spec())
    }
    fn explain(&self) -> String {
        self.inner.explain()
    }
    fn _browser_sources<'py>(&self, py: Python<'py>) -> Vec<(u64, &str, Bound<'py, PyBytes>)> {
        self.browser_sources
            .iter()
            .map(|(identity, name, bytes)| (*identity, name.as_str(), PyBytes::new(py, bytes)))
            .collect()
    }
}

#[pymodule]
fn _engine(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add("MolgfxError", module.py().get_type::<MolgfxError>())?;
    module.add("SpecError", module.py().get_type::<SpecError>())?;
    module.add(
        "RevisionConflict",
        module.py().get_type::<RevisionConflict>(),
    )?;
    module.add_class::<PyRepresentation>()?;
    module.add_class::<PySceneSpec>()?;
    module.add_class::<PyScenePatch>()?;
    module.add_class::<PyScene>()?;
    crate::transaction_binding::register(module)?;
    crate::authoring_binding::register(module)?;
    crate::render_binding::register(module)?;
    crate::visual_binding::register(module)?;
    let rep = PyModule::new(module.py(), "rep")?;
    rep.add_function(wrap_pyfunction!(cartoon, &rep)?)?;
    rep.add_function(wrap_pyfunction!(ball_and_stick, &rep)?)?;
    rep.add_function(wrap_pyfunction!(spacefill, &rep)?)?;
    rep.add_function(wrap_pyfunction!(licorice, &rep)?)?;
    rep.add_function(wrap_pyfunction!(lines, &rep)?)?;
    rep.add_function(wrap_pyfunction!(points, &rep)?)?;
    rep.add_function(wrap_pyfunction!(surface, &rep)?)?;
    rep.add_function(wrap_pyfunction!(nucleic_acid, &rep)?)?;
    rep.add_function(wrap_pyfunction!(bases, &rep)?)?;
    rep.add_function(wrap_pyfunction!(base_pairs, &rep)?)?;
    rep.add_function(wrap_pyfunction!(glycan, &rep)?)?;
    module.add_submodule(&rep)
}
