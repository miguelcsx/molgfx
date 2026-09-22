//! The intentionally small Python surface.
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

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
#[pyclass(name = "Representation", frozen, skip_from_py_object)]
pub(super) struct PyRepresentation(pub(super) molgfx::RepresentationSpec);

impl PyRepresentation {
    pub(super) fn add_to(
        &self,
        scene: &mut molgfx::Scene,
    ) -> Result<molgfx::RepresentationId, molgfx::Error> {
        scene.add(self.0.clone())
    }

    fn with_visual(&self, visual: molgfx::VisualStyle) -> Self {
        Self(self.0.clone().visual(visual))
    }

    fn from_item(item: impl Into<molgfx::RepresentationSpec>) -> Self {
        Self(item.into())
    }
}

#[pymethods]
impl PyRepresentation {
    fn explain(&self) -> String {
        self.0.explain()
    }

    fn visual(&self, style: &crate::visual_binding::PyVisualStyle) -> Self {
        self.with_visual(style.0.clone())
    }
}

#[derive(Clone, Debug)]
#[pyclass(name = "SceneSpec", frozen, skip_from_py_object)]
pub(super) struct PySceneSpec(pub(super) molgfx::SceneSpec);

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
pub(super) struct PyScenePatch(pub(super) molgfx::ScenePatch);

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

/// Converts a Python-facing control value into the builder's own type.
///
/// Enumerated controls arrive from Python as strings; numeric ones pass
/// through. Keeping the conversion in a trait lets one declaration below
/// describe every representation regardless of how its controls are typed.
trait Control<T> {
    fn control(self) -> PyResult<T>;
}

impl Control<f32> for f32 {
    fn control(self) -> PyResult<f32> {
        Ok(self)
    }
}

/// Declares the string spellings of one enumerated control.
macro_rules! named_control {
    ($type:ty, $label:literal, [$($name:literal => $variant:ident),* $(,)?]) => {
        impl Control<$type> for String {
            fn control(self) -> PyResult<$type> {
                match self.as_str() {
                    $($name => Ok(<$type>::$variant),)*
                    other => Err(PyValueError::new_err(format!(
                        "unknown {} '{other}'; expected one of {}",
                        $label,
                        [$($name),*].join(", ")
                    ))),
                }
            }
        }
    };
}

named_control!(molgfx::rep::CartoonStyle, "cartoon style", [
    "ribbon" => Ribbon,
    "rocket" => Rocket,
    "nucleic_acid" => NucleicAcid,
    "glycan" => Glycan,
]);

named_control!(molgfx::rep::SurfaceKind, "surface kind", [
    "van_der_waals" => VanDerWaals,
    "solvent_accessible" => SolventAccessible,
    "solvent_excluded" => SolventExcluded,
    "gaussian" => Gaussian,
]);

named_control!(molgfx::rep::SurfaceStyle, "surface style", [
    "solid" => Solid,
    "contour" => Contour,
    "dots" => Dots,
    "filled_contour" => FilledContour,
    "mesh" => Mesh,
]);

/// Declares one representation constructor over exactly the controls its form
/// defines.
///
/// The list mirrors the typed Rust builder for the same form, so a control that
/// exists in one language exists in the other and neither can drift into
/// offering a parameter the renderer has no place to put.
macro_rules! representation {
    ($name:ident $(, $control:ident : $type:ty)* $(,)?) => {
        #[pyfunction]
        #[pyo3(signature = (*, target, $($control = None,)* opacity = 1.0, color = None))]
        fn $name(
            target: &Bound<'_, PyAny>,
            $($control: Option<$type>,)*
            opacity: f32,
            color: Option<&Bound<'_, PyAny>>,
        ) -> PyResult<PyRepresentation> {
            let mut value = molgfx::rep::$name(selection(target)?).opacity(opacity);
            $(
                if let Some($control) = $control {
                    value = value.$control(Control::control($control)?);
                }
            )*
            if let Some(color) = color_spec(color)? {
                value = value.color(color);
            }
            Ok(PyRepresentation::from_item(value))
        }
    };
}

representation!(cartoon, width: f32, style: String);
representation!(ball_and_stick, radius: f32, bond_radius: f32);
representation!(spacefill, radius: f32);
representation!(licorice, radius: f32, bond_radius: f32);
representation!(lines, width: f32);
representation!(points, size: f32);
representation!(
    surface,
    kind: String,
    style: String,
    probe_radius: f32,
    isolevel: f32
);
representation!(nucleic_acid, width: f32);
representation!(bases, radius: f32);
representation!(base_pairs, radius: f32, bond_radius: f32);
representation!(glycan, width: f32);

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
    module.add_class::<crate::scene_binding::PyScene>()?;
    crate::id_binding::register(module)?;
    crate::transaction_binding::register(module)?;
    crate::authoring_binding::register(module)?;
    crate::render_binding::register(module)?;
    crate::science_binding::register(module)?;
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
