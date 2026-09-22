//! Mutable Python scene and browser-source transport.

use crate::binding::{PyRepresentation, PyScenePatch, PySceneSpec, error, selection};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes};
use std::sync::Arc;

#[pyclass(name = "Scene")]
pub(super) struct PyScene {
    pub(super) inner: molgfx::Scene,
    pub(super) pending: Option<Vec<molgfx::schema::PatchOperation>>,
    structure_id: u64,
    browser_sources: Vec<BrowserSource>,
    subscribers: Vec<Py<PyAny>>,
}

struct BrowserSource {
    identity: u64,
    name: String,
    provider: crate::native_adapter::SharedNativeProvider,
    encoded: Option<Vec<u8>>,
}

impl PyScene {
    pub(super) fn stage_or_apply(
        &mut self,
        py: Python<'_>,
        operation: molgfx::schema::PatchOperation,
    ) -> PyResult<()> {
        if let Some(pending) = &mut self.pending {
            pending.push(operation);
            return Ok(());
        }
        let patch = molgfx::ScenePatch {
            base_revision: self.inner.revision(),
            operations: vec![operation],
        };
        self.inner.apply(&patch).map_err(error)?;
        self.publish(py, &patch)
    }

    pub(super) fn publish(&mut self, py: Python<'_>, patch: &molgfx::ScenePatch) -> PyResult<()> {
        let encoded = patch.to_json().map_err(error)?;
        let mut live = Vec::with_capacity(self.subscribers.len());
        for reference in self.subscribers.drain(..) {
            let callback = reference.call0(py)?;
            if callback.bind(py).is_none() {
                continue;
            }
            let _ = callback.call1(py, (&encoded,))?;
            live.push(reference);
        }
        self.subscribers = live;
        Ok(())
    }
}

#[pymethods]
impl PyScene {
    #[new]
    fn new(structure: &Bound<'_, PyAny>) -> PyResult<Self> {
        let provider = Arc::new(crate::native_adapter::NativeProvider::import(structure)?);
        let source = crate::native_adapter::source(&provider);
        let inner = molgfx::Scene::from_source(source).map_err(error)?;
        Ok(Self {
            inner,
            pending: None,
            structure_id: 1,
            browser_sources: vec![BrowserSource {
                identity: 1,
                name: "structure.bcif".to_owned(),
                provider,
                encoded: None,
            }],
            subscribers: Vec::new(),
        })
    }

    fn add(&mut self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "scene items cannot be added inside a scene transaction",
            ));
        }
        let base_revision = self.inner.revision();
        let (id, operation) = if let Ok(representation) =
            item.extract::<PyRef<'_, PyRepresentation>>()
        {
            let id = representation.add_to(&mut self.inner).map_err(error)?;
            let inserted = self
                .inner
                .spec()
                .representations
                .get(&id)
                .cloned()
                .ok_or_else(|| PyValueError::new_err("inserted representation is unavailable"))?;
            (
                crate::id_binding::PySceneId::Representation(id.get()),
                molgfx::schema::PatchOperation::AddRepresentation {
                    id,
                    representation: inserted,
                },
            )
        } else {
            crate::science_binding::add_item(item, &mut self.inner)?
        };
        self.publish(
            py,
            &molgfx::ScenePatch {
                base_revision,
                operations: vec![operation],
            },
        )?;
        id.into_python(py)
    }

    #[pyo3(signature = (*, structure, name, source_hash, values, units=None, domain=None))]
    fn bind_property(
        &mut self,
        structure: &Bound<'_, PyAny>,
        name: &str,
        source_hash: &str,
        values: Vec<f32>,
        units: Option<&str>,
        domain: Option<(f32, f32)>,
    ) -> PyResult<crate::authoring_binding::PyScalarProperty> {
        let mut binding = molgfx::ScalarPropertyBinding::new(
            molgfx::StructureId::new(crate::id_binding::structure_id(structure)?),
            name,
            molgfx::schema::DataSource::new(source_hash),
            Arc::from(values),
        );
        if let Some(units) = units {
            binding = binding.units(units);
        }
        if let Some((low, high)) = domain {
            binding = binding.domain([low, high]);
        }
        self.inner
            .bind_property(binding)
            .map(crate::authoring_binding::PyScalarProperty)
            .map_err(error)
    }

    fn set_visible(
        &mut self,
        py: Python<'_>,
        representation: &Bound<'_, PyAny>,
        visible: bool,
    ) -> PyResult<()> {
        self.stage_or_apply(
            py,
            molgfx::schema::PatchOperation::SetVisibility {
                id: molgfx::RepresentationId::new(crate::id_binding::representation_id(
                    representation,
                )?),
                visible,
            },
        )
    }

    fn set_opacity(
        &mut self,
        py: Python<'_>,
        representation: &Bound<'_, PyAny>,
        opacity: f32,
    ) -> PyResult<()> {
        self.stage_or_apply(
            py,
            molgfx::schema::PatchOperation::SetOpacity {
                id: molgfx::RepresentationId::new(crate::id_binding::representation_id(
                    representation,
                )?),
                opacity,
            },
        )
    }

    fn set_visual(
        &mut self,
        py: Python<'_>,
        representation: &Bound<'_, PyAny>,
        visual: Option<&crate::visual_binding::PyVisualStyle>,
    ) -> PyResult<()> {
        self.stage_or_apply(
            py,
            molgfx::schema::PatchOperation::SetVisual {
                id: molgfx::RepresentationId::new(crate::id_binding::representation_id(
                    representation,
                )?),
                visual: visual.map(|value| value.0.clone()),
            },
        )
    }

    fn focus(&mut self, py: Python<'_>, target: &Bound<'_, PyAny>) -> PyResult<()> {
        self.stage_or_apply(
            py,
            molgfx::schema::PatchOperation::SetFocus {
                selection: Some(selection(target)?.into()),
            },
        )
    }

    fn apply(&mut self, py: Python<'_>, patch: &PyScenePatch) -> PyResult<()> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "patches cannot be applied inside a scene transaction",
            ));
        }
        self.inner.apply(&patch.0).map_err(error)?;
        self.publish(py, &patch.0)
    }

    #[getter]
    fn revision(&self) -> u64 {
        self.inner.revision()
    }

    #[getter]
    fn structure_id(&self) -> crate::id_binding::PyStructureId {
        crate::id_binding::PyStructureId(self.structure_id)
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

    fn _browser_sources<'py>(
        &mut self,
        py: Python<'py>,
    ) -> PyResult<Vec<(u64, &str, Bound<'py, PyBytes>)>> {
        for source in &mut self.browser_sources {
            if source.encoded.is_none() {
                source.encoded = Some(source.provider.browser_bytes()?);
            }
        }
        Ok(self
            .browser_sources
            .iter()
            .filter_map(|source| {
                source.encoded.as_ref().map(|bytes| {
                    (
                        source.identity,
                        source.name.as_str(),
                        PyBytes::new(py, bytes),
                    )
                })
            })
            .collect())
    }

    fn _subscribe(&mut self, subscriber: Py<PyAny>) {
        self.subscribers.push(subscriber);
    }
}
