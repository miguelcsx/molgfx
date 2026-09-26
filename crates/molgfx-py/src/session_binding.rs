//! A Python authoring session sharing one live scene.

use crate::binding::PyScenePatch;
use crate::command_binding::{PyCommand, command_error};
use crate::scene_binding::PyScene;
use molgfx::command::{self, Program};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyModule};

/// What one execution did.
#[derive(Debug)]
#[pyclass(name = "CommandResult", frozen)]
pub(super) struct PyCommandResult {
    #[pyo3(get)]
    revision: u64,
    #[pyo3(get)]
    messages: Vec<String>,
    patch: Option<molgfx::ScenePatch>,
}

#[pymethods]
impl PyCommandResult {
    /// The patch applied to the scene, or `None` when only names changed.
    #[getter]
    fn patch(&self) -> Option<PyScenePatch> {
        self.patch.clone().map(PyScenePatch)
    }

    fn __repr__(&self) -> String {
        let operations = self
            .patch
            .as_ref()
            .map_or(0, |patch| patch.operations.len());
        format!(
            "CommandResult(revision={}, operations={operations}, messages={:?})",
            self.revision, self.messages
        )
    }
}

/// Names, colour rules and history for authoring one scene with commands.
#[pyclass(name = "Session")]
pub(super) struct PySession {
    scene: Py<PyScene>,
    inner: command::Session,
}

impl PySession {
    fn run(&mut self, py: Python<'_>, program: &Program) -> PyResult<PyCommandResult> {
        let outcome = {
            let mut scene = self.scene.borrow_mut(py);
            if scene.pending.is_some() {
                return Err(PyValueError::new_err(
                    "commands cannot run inside a scene transaction",
                ));
            }
            self.inner
                .execute(&mut scene.inner, program)
                .map_err(|errors| command_error(py, &errors, program.source()))?
        };
        if let Some(patch) = &outcome.patch {
            crate::scene_binding::deliver(self.scene.bind(py), py, patch)?;
        }
        Ok(PyCommandResult {
            revision: outcome.revision,
            messages: outcome.messages,
            patch: outcome.patch,
        })
    }
}

#[pymethods]
impl PySession {
    /// Starts a session over a `Scene`, or over a new scene of a structure.
    #[new]
    fn new(py: Python<'_>, source: &Bound<'_, PyAny>) -> PyResult<Self> {
        let scene: Py<PyScene> = match source.cast::<PyScene>() {
            Ok(scene) => scene.clone().unbind(),
            Err(_) => Py::new(py, PyScene::for_structure(source)?)?,
        };
        let inner = command::Session::new(&scene.borrow(py).inner);
        Ok(Self { scene, inner })
    }

    /// The live scene this session edits, shared with every viewer of it.
    #[getter]
    fn scene(&self, py: Python<'_>) -> Py<PyScene> {
        self.scene.clone_ref(py)
    }

    /// Executes command text or a typed `Command` as one atomic edit.
    fn execute(&mut self, py: Python<'_>, program: &Bound<'_, PyAny>) -> PyResult<PyCommandResult> {
        if let Ok(command) = program.extract::<PyRef<'_, PyCommand>>() {
            let program = Program::from(command.0.clone());
            return self.run(py, &program);
        }
        if let Ok(text) = program.extract::<String>() {
            let parsed =
                Program::parse(&text).map_err(|errors| command_error(py, &errors, Some(&text)))?;
            return self.run(py, &parsed);
        }
        if let Ok(commands) = program.extract::<Vec<PyRef<'_, PyCommand>>>() {
            let program = Program::from_commands(commands.iter().map(|command| command.0.clone()));
            return self.run(py, &program);
        }
        Err(PyTypeError::new_err(
            "execute takes command text, a Command, or a list of Commands",
        ))
    }

    /// Parses command text and returns its canonical form without running it.
    #[staticmethod]
    fn explain(py: Python<'_>, text: &str) -> PyResult<String> {
        Program::parse(text)
            .map(|program| program.explain())
            .map_err(|errors| command_error(py, &errors, Some(text)))
    }

    fn undo(&mut self, py: Python<'_>) -> PyResult<PyCommandResult> {
        self.run(py, &Program::from(command::Command::Undo))
    }

    fn redo(&mut self, py: Python<'_>) -> PyResult<PyCommandResult> {
        self.run(py, &Program::from(command::Command::Redo))
    }

    #[getter]
    fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    #[getter]
    fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    /// Labels of the edits undo would reverse, oldest first.
    #[getter]
    fn history(&self) -> Vec<String> {
        self.inner.history()
    }

    /// Named selections as declared.
    #[getter]
    fn selections<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (name, query) in &self.inner.spec().selections {
            out.set_item(name.as_str(), query.source())?;
        }
        Ok(out)
    }

    /// Layers as `{name: (form, target, representation_id)}`.
    #[getter]
    fn layers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (name, layer) in &self.inner.spec().layers {
            out.set_item(
                name.as_str(),
                (
                    layer.form.kind().name(),
                    layer.target.source(),
                    layer.id.get(),
                ),
            )?;
        }
        Ok(out)
    }

    /// Structure names as `{name: structure_id}`.
    #[getter]
    fn structures<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (name, id) in &self.inner.spec().structures {
            out.set_item(name.as_str(), id.get())?;
        }
        Ok(out)
    }

    /// Completion candidates as `(text, kind, detail)` for the word ending at
    /// `cursor`, which defaults to the end of `text`.
    #[pyo3(signature = (text, cursor=None))]
    fn completions(
        &self,
        text: &str,
        cursor: Option<usize>,
    ) -> Vec<(String, &'static str, String)> {
        self.inner
            .completions(
                text,
                cursor.map_or(text.len(), |cursor| cursor.min(text.len())),
            )
            .into_iter()
            .map(|completion| (completion.text, completion.kind, completion.detail))
            .collect()
    }

    fn explain_selection(&self, name: &str) -> PyResult<String> {
        self.inner
            .explain_selection(name)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    fn explain_layer(&self, name: &str) -> PyResult<String> {
        self.inner
            .explain_layer(name)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    /// Every name the session knows, one per line.
    fn summary(&self) -> String {
        self.inner.summary()
    }

    /// The session's names as JSON, to restore with `Session.from_json`.
    fn to_json(&self) -> PyResult<String> {
        self.inner
            .spec()
            .to_json()
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    /// Restores a session's names over an existing scene.
    #[staticmethod]
    fn from_json(py: Python<'_>, scene: Py<PyScene>, source: &str) -> PyResult<Self> {
        let spec = command::SessionSpec::from_json(source)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let inner = command::Session::from_spec(spec, &scene.borrow(py).inner);
        Ok(Self { scene, inner })
    }

    fn __repr__(&self) -> String {
        let spec = self.inner.spec();
        format!(
            "Session(structures={}, selections={}, layers={})",
            spec.structures.len(),
            spec.selections.len(),
            spec.layers.len()
        )
    }
}

/// The command vocabulary, for completion menus and help.
#[pyfunction]
fn vocabulary(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let out = PyDict::new(py);
    let verbs: Vec<(&str, &str, &str)> = command::registry::VERBS
        .iter()
        .map(|verb| (verb.name, verb.synopsis, verb.summary))
        .collect();
    out.set_item("verbs", verbs)?;
    let forms = PyDict::new(py);
    for kind in command::FormKind::ALL {
        let controls: Vec<(&str, &str)> = kind
            .options()
            .iter()
            .map(|option| (option.name, option.summary))
            .collect();
        forms.set_item(kind.name(), controls)?;
    }
    out.set_item("forms", forms)?;
    out.set_item("schemes", command::registry::SCHEMES.to_vec())?;
    out.set_item(
        "colors",
        command::registry::NAMED_COLORS
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
    )?;
    out.set_item("ramps", command::registry::RAMPS.to_vec())?;
    Ok(out)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySession>()?;
    module.add_class::<PyCommandResult>()?;
    module.add_function(wrap_pyfunction!(vocabulary, module)?)
}
