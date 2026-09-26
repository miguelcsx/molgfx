//! Typed authoring commands for Python.

use molgfx::command::{self, ColorValue, Form, FormKind, Name, Opacity, QueryText, Show, Target};
use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

create_exception!(_engine, CommandError, crate::binding::MolgfxError);

/// Raises `CommandError` whose `errors` attribute lists every located error.
pub(super) fn command_error(
    py: Python<'_>,
    errors: &command::CommandErrors,
    source: Option<&str>,
) -> PyErr {
    let message = source.map_or_else(|| errors.to_string(), |source| errors.render(source));
    let raised = CommandError::new_err(message);
    let details = PyList::empty(py);
    for error in errors.as_slice() {
        let entry = PyDict::new(py);
        let filled = (|| -> PyResult<()> {
            entry.set_item(
                "kind",
                serde_json::to_value(error.kind)
                    .ok()
                    .and_then(|kind| kind.as_str().map(str::to_owned)),
            )?;
            entry.set_item("message", &error.message)?;
            entry.set_item("span", error.span.map(|span| (span.start, span.end)))?;
            entry.set_item("statement", error.statement)?;
            entry.set_item("suggestion", &error.suggestion)?;
            entry.set_item("code", &error.code)?;
            details.append(entry)
        })();
        if let Err(problem) = filled {
            return problem;
        }
    }
    match raised.value(py).setattr("errors", details) {
        Ok(()) => raised,
        Err(problem) => problem,
    }
}

fn invalid(message: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(message.to_string())
}

fn name(value: &str) -> PyResult<Name> {
    Name::new(value).map_err(|reason| invalid(format!("'{value}' is not a valid name: {reason}")))
}

fn layer(value: &str) -> PyResult<Name> {
    name(value.strip_prefix('@').map_or(value, str::trim_start))
}

fn query(py: Python<'_>, text: &str) -> PyResult<QueryText> {
    QueryText::compile(text).map_err(|diagnostics| {
        let located = diagnostics.first().map_or_else(
            || "the query is not valid".to_owned(),
            |first| first.message().to_owned(),
        );
        let error = command::CommandError::new(command::ErrorKind::Query, located);
        command_error(py, &command::CommandErrors::one(error), None)
    })
}

/// A target given as `@layer`, a query string, or a `molframe` query.
fn target(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Target> {
    let text = crate::binding::selection(value)?;
    match text.strip_prefix('@') {
        Some(layer_name) => name(layer_name).map(Target::Layer),
        None => query(py, &text).map(Target::Query),
    }
}

fn color(value: &str) -> PyResult<ColorValue> {
    if value.starts_with('#') {
        return ColorValue::hex(value).map_err(invalid);
    }
    ColorValue::named(value).ok_or_else(|| {
        let near = command::registry::suggest(value, command::registry::color_words());
        invalid(match near {
            Some(near) => format!("'{value}' is not a colour; did you mean '{near}'?"),
            None => format!("'{value}' is not a colour"),
        })
    })
}

fn structure(value: Option<&str>) -> PyResult<Option<Name>> {
    value.map(name).transpose()
}

/// Sets one keyword of `Command.show`.
fn show_option(show: &mut Show, key: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
    match key {
        "layer" => show.layer = Some(name(&value.extract::<String>()?)?),
        "structure" => show.structure = Some(name(&value.extract::<String>()?)?),
        "color" => show.color = Some(color(&value.extract::<String>()?)?),
        "opacity" => show.opacity = Some(Opacity::new(value.extract::<f32>()?).map_err(invalid)?),
        "duplicate" => show.duplicate = value.extract::<bool>()?,
        control => {
            let text = value.str()?.to_string();
            let form = show.form.kind().name();
            show.form
                .set_option(control, &text)
                .map_err(|problem| match problem {
                    command::OptionError::Unknown { known } => invalid(format!(
                        "{form} has no control '{control}'; its controls are {}",
                        known
                            .iter()
                            .map(|option| option.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                    command::OptionError::Value(reason) => invalid(format!("{control}: {reason}")),
                })?;
        }
    }
    Ok(())
}

/// One validated authoring command.
#[derive(Clone, Debug)]
#[pyclass(name = "Command", frozen, skip_from_py_object)]
pub(super) struct PyCommand(pub(super) command::Command);

#[pymethods]
impl PyCommand {
    /// Parses one statement of command text.
    #[staticmethod]
    fn parse(py: Python<'_>, text: &str) -> PyResult<Self> {
        let program = command::Program::parse(text)
            .map_err(|errors| command_error(py, &errors, Some(text)))?;
        match program.statements() {
            [statement] => Ok(Self(statement.command.clone())),
            _ => Err(invalid("expected exactly one statement")),
        }
    }

    /// Reads a command from its JSON form.
    #[staticmethod]
    fn from_json(source: &str) -> PyResult<Self> {
        serde_json::from_str(source).map(Self).map_err(invalid)
    }

    #[staticmethod]
    fn select(py: Python<'_>, selection: &str, target: &Bound<'_, PyAny>) -> PyResult<Self> {
        let text = crate::binding::selection(target)?;
        Ok(Self(command::Command::Select {
            name: name(selection)?,
            query: query(py, &text)?,
        }))
    }

    #[staticmethod]
    fn unselect(selection: &str) -> PyResult<Self> {
        Ok(Self(command::Command::Unselect {
            name: name(selection)?,
        }))
    }

    /// Draws a target with a form.
    ///
    /// Keywords `layer`, `structure`, `color`, `opacity` and `duplicate` set
    /// the layer; every other keyword is one of the form's own controls.
    #[staticmethod]
    #[pyo3(signature = (form, target, **options))]
    fn show(
        py: Python<'_>,
        form: &str,
        target: &Bound<'_, PyAny>,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let kind = FormKind::from_name(form).ok_or_else(|| {
            let near = command::registry::suggest(form, command::registry::form_names());
            invalid(match near {
                Some(near) => format!("'{form}' is not a form; did you mean '{near}'?"),
                None => format!("'{form}' is not a form"),
            })
        })?;
        let mut show = Show::new(Form::new(kind), self::target(py, target)?);
        if let Some(options) = options {
            for (key, value) in options.iter() {
                let key: String = key.extract()?;
                show_option(&mut show, &key, &value)?;
            }
        }
        Ok(Self(command::Command::Show(show)))
    }

    #[staticmethod]
    fn reveal(layer: &str) -> PyResult<Self> {
        Ok(Self(command::Command::Reveal {
            layer: self::layer(layer)?,
        }))
    }

    #[staticmethod]
    fn hide(layer: &str) -> PyResult<Self> {
        Ok(Self(command::Command::Hide {
            layer: self::layer(layer)?,
        }))
    }

    #[staticmethod]
    fn remove(layer: &str) -> PyResult<Self> {
        Ok(Self(command::Command::Remove {
            layer: self::layer(layer)?,
        }))
    }

    #[staticmethod]
    #[pyo3(signature = (color, target, *, structure=None))]
    fn color(
        py: Python<'_>,
        color: &str,
        target: &Bound<'_, PyAny>,
        structure: Option<&str>,
    ) -> PyResult<Self> {
        Ok(Self(command::Command::Color {
            color: self::color(color)?,
            target: self::target(py, target)?,
            structure: self::structure(structure)?,
        }))
    }

    #[staticmethod]
    #[pyo3(signature = (target=None, *, structure=None))]
    fn uncolor(
        py: Python<'_>,
        target: Option<&Bound<'_, PyAny>>,
        structure: Option<&str>,
    ) -> PyResult<Self> {
        let target = match target {
            Some(target) => Some(query(py, &crate::binding::selection(target)?)?),
            None => None,
        };
        Ok(Self(command::Command::Uncolor {
            target,
            structure: self::structure(structure)?,
        }))
    }

    #[staticmethod]
    fn opacity(value: f32, layer: &str) -> PyResult<Self> {
        Ok(Self(command::Command::Opacity {
            value: Opacity::new(value).map_err(invalid)?,
            layer: self::layer(layer)?,
        }))
    }

    #[staticmethod]
    fn focus(py: Python<'_>, target: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self(command::Command::Focus {
            target: self::target(py, target)?,
        }))
    }

    #[staticmethod]
    fn unfocus() -> Self {
        Self(command::Command::Unfocus)
    }

    #[staticmethod]
    fn undo() -> Self {
        Self(command::Command::Undo)
    }

    #[staticmethod]
    fn redo() -> Self {
        Self(command::Command::Redo)
    }

    /// The command's verb.
    #[getter]
    fn verb(&self) -> &'static str {
        self.0.verb()
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.0).map_err(invalid)
    }

    fn __str__(&self) -> String {
        self.0.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Command({:?})", self.0.to_string())
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("CommandError", module.py().get_type::<CommandError>())?;
    module.add_class::<PyCommand>()
}
