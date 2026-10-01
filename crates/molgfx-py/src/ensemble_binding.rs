//! Ensemble overlays and property-difference views on the Python scene.

use crate::binding::selection;
use crate::scene_binding::PyScene;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// One overlay member as Python writes it: structure, weight, colour.
type Member<'py> = (Bound<'py, PyAny>, f32, (u8, u8, u8));

fn rgb(value: (u8, u8, u8)) -> molgfx::Color {
    molgfx::Color::rgb(value.0, value.1, value.2)
}

#[pymethods]
impl PyScene {
    /// Overlays several structures of this scene, the heaviest most opaque.
    ///
    /// Each member is `(structure, weight, (r, g, b))`. The heaviest member
    /// draws at `dominant_opacity`; the others scale `alternate_opacity` by
    /// their weight relative to it, never below `minimum_opacity`.
    #[pyo3(signature = (members, *, dominant_opacity=None, alternate_opacity=None, minimum_opacity=None))]
    fn ensemble(
        &mut self,
        py: Python<'_>,
        members: Vec<Member<'_>>,
        dominant_opacity: Option<f32>,
        alternate_opacity: Option<f32>,
        minimum_opacity: Option<f32>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "an ensemble cannot be added inside a scene transaction",
            ));
        }
        let members = members
            .into_iter()
            .map(|(structure, weight, color)| {
                Ok(molgfx::preset::EnsembleMember {
                    structure: molgfx::StructureId::new(crate::id_binding::structure_id(
                        &structure,
                    )?),
                    weight,
                    color: rgb(color),
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        let mut style = molgfx::preset::EnsembleStyle::default();
        if let Some(value) = dominant_opacity {
            style.dominant_opacity = value;
        }
        if let Some(value) = alternate_opacity {
            style.alternate_opacity = value;
        }
        if let Some(value) = minimum_opacity {
            style.minimum_opacity = value;
        }
        let base_revision = self.inner.revision();
        let inserted = self
            .inner
            .add_ensemble(&members, style)
            .map_err(crate::binding::error)?;
        self.publish_added(py, base_revision, inserted, Vec::new())
    }

    /// Draws `target` as a cartoon coloured by a bound property, with small
    /// values faded to context. See `DifferenceStyle` for the mapping.
    #[pyo3(signature = (property, target, *, structure=None, style=None))]
    fn difference(
        &mut self,
        py: Python<'_>,
        property: &crate::authoring_binding::PyScalarProperty,
        target: &Bound<'_, PyAny>,
        structure: Option<&Bound<'_, PyAny>>,
        style: Option<PyRef<'_, PyDifferenceStyle>>,
    ) -> PyResult<Py<PyAny>> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "a difference view cannot be added inside a scene transaction",
            ));
        }
        let structure = match structure {
            Some(structure) => crate::id_binding::structure_id(structure)?,
            None => self.structure_id,
        };
        let style = style.map_or_else(molgfx::preset::DifferenceStyle::default, |style| {
            style.0.clone()
        });
        let target = molgfx::Selection::from(selection(target)?);
        let base_revision = self.inner.revision();
        let inserted = self
            .inner
            .add_difference(
                molgfx::StructureId::new(structure),
                target,
                property.0.clone(),
                &style,
            )
            .map_err(crate::binding::error)?;
        let mut ids = self.publish_added(py, base_revision, vec![inserted], Vec::new())?;
        ids.pop()
            .ok_or_else(|| PyValueError::new_err("the difference view was not added"))
    }
}

/// Thresholds, opacity and palette of a property-difference view.
///
/// Opacity rises linearly from `context_opacity` at `thresholds[0]` to one at
/// `thresholds[1]`; the palette spans `domain`. Validation happens when the
/// style is applied to a scene.
#[derive(Clone, Debug)]
#[pyclass(name = "DifferenceStyle", frozen, from_py_object)]
pub(super) struct PyDifferenceStyle(molgfx::preset::DifferenceStyle);

#[pymethods]
impl PyDifferenceStyle {
    #[new]
    #[pyo3(signature = (*, thresholds=None, context_opacity=None, palette=None, domain=None, missing=None))]
    fn new(
        thresholds: Option<(f32, f32)>,
        context_opacity: Option<f32>,
        palette: Option<&str>,
        domain: Option<(f32, f32)>,
        missing: Option<(u8, u8, u8)>,
    ) -> Self {
        let mut style = molgfx::preset::DifferenceStyle::default();
        if let Some((context, emphasis)) = thresholds {
            style.context_threshold = context;
            style.emphasis_threshold = emphasis;
        }
        if let Some(opacity) = context_opacity {
            style.context_opacity = opacity;
        }
        if let Some(palette) = palette {
            palette.clone_into(&mut style.palette);
        }
        if let Some((low, high)) = domain {
            style.domain = [low, high];
        }
        if let Some(color) = missing {
            style.missing = rgb(color);
        }
        Self(style)
    }

    #[getter]
    fn thresholds(&self) -> (f32, f32) {
        (self.0.context_threshold, self.0.emphasis_threshold)
    }

    #[getter]
    fn context_opacity(&self) -> f32 {
        self.0.context_opacity
    }

    #[getter]
    fn palette(&self) -> &str {
        &self.0.palette
    }

    #[getter]
    fn domain(&self) -> (f32, f32) {
        (self.0.domain[0], self.0.domain[1])
    }

    fn __repr__(&self) -> String {
        format!(
            "DifferenceStyle(thresholds=({}, {}), palette={:?})",
            self.0.context_threshold, self.0.emphasis_threshold, self.0.palette
        )
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyDifferenceStyle>()
}
