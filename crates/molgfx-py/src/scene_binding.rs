//! Mutable Python scene and browser-source transport.

use crate::binding::{PyRepresentation, PyScenePatch, PySceneSpec, error, selection};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes};
use std::sync::{Arc, OnceLock};

#[pyclass(name = "Scene")]
pub(super) struct PyScene {
    pub(super) inner: molgfx::Scene,
    pub(super) pending: Option<Vec<molgfx::schema::PatchOperation>>,
    pub(super) structure_id: u64,
    pub(super) browser_sources: Vec<BrowserSource>,
    subscribers: Vec<Py<PyAny>>,
}

pub(super) struct BrowserSource {
    pub(super) identity: u64,
    pub(super) name: String,
    pub(super) provider: crate::native_adapter::SharedNativeProvider,
    /// Compact `BinaryCIF` encoding, produced on the first browser transfer.
    pub(super) encoded: Arc<OnceLock<Vec<u8>>>,
}

impl PyScene {
    /// A scene over one Python `molframe` structure.
    pub(super) fn for_structure(structure: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::new(Some(structure))
    }

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
        deliver_subscribers(py, &mut self.subscribers, patch)
    }
}

/// Calls every live weak subscriber with one encoded patch.
///
/// Dropped weak references are pruned; `Subscriber callback error` propagates.
fn deliver_subscribers(
    py: Python<'_>,
    subscribers: &mut Vec<Py<PyAny>>,
    patch: &molgfx::ScenePatch,
) -> PyResult<()> {
    let encoded = patch.to_json().map_err(error)?;
    let mut index = 0;
    while index < subscribers.len() {
        let callback = subscribers[index].call0(py)?;
        if callback.bind(py).is_none() {
            subscribers.remove(index);
        } else {
            let _ = callback.call1(py, (&encoded,))?;
            index += 1;
        }
    }
    Ok(())
}

/// Announces one patch to subscribers without holding a borrow of the scene.
pub(super) fn deliver(
    slf: &Bound<'_, PyScene>,
    py: Python<'_>,
    patch: &molgfx::ScenePatch,
) -> PyResult<()> {
    let encoded = patch.to_json().map_err(error)?;
    // Keep the authoritative list in the scene while callbacks run: a callback
    // can unsubscribe itself, subscribe another view, or publish a nested edit.
    let subscribers: Vec<_> = slf
        .borrow()
        .subscribers
        .iter()
        .map(|reference| reference.clone_ref(py))
        .collect();
    for reference in subscribers {
        if !slf
            .borrow()
            .subscribers
            .iter()
            .any(|current| current.bind(py).is(reference.bind(py)))
        {
            continue;
        }
        let callback = reference.call0(py)?;
        if callback.bind(py).is_none() {
            slf.borrow_mut()
                .subscribers
                .retain(|current| !current.bind(py).is(reference.bind(py)));
        } else {
            let _ = callback.call1(py, (&encoded,))?;
        }
    }
    Ok(())
}

#[pymethods]
impl PyScene {
    #[new]
    #[pyo3(signature = (structure=None))]
    fn new(structure: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let Some(structure) = structure else {
            return Ok(Self {
                inner: molgfx::Scene::empty(),
                pending: None,
                structure_id: 0,
                browser_sources: Vec::new(),
                subscribers: Vec::new(),
            });
        };
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
                encoded: Arc::new(OnceLock::new()),
            }],
            subscribers: Vec::new(),
        })
    }

    /// Binds the scalar values for an authored volume identity.
    fn bind_volume(&mut self, identity: &Bound<'_, PyAny>, values: Vec<f32>) -> PyResult<()> {
        let identity = identity.extract::<PyRef<'_, crate::id_binding::PyVolumeId>>()?;
        let spec = self
            .inner
            .spec()
            .volumes
            .get(&molgfx::VolumeId::new(identity.0))
            .ok_or_else(|| PyValueError::new_err("unknown volume identity"))?;
        self.inner
            .bind_volume(
                molgfx::VolumeBinding::new(spec.source.clone(), spec.dimensions, Arc::from(values))
                    .affine(spec.voxel_to_world),
            )
            .map_err(error)
    }

    fn unresolved_overlays(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.unresolved_overlays())
            .map_err(|err| PyValueError::new_err(err.to_string()))
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
            crate::overlay_binding::add_item(item, &mut self.inner)?
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

    /// Binds the two resident frames a trajectory descriptor samples between.
    ///
    /// The pair replaces whatever interval the structure held, and the sample
    /// time is clamped to it.
    #[pyo3(signature = (*, source_hash, start, end, sample_time=None))]
    fn bind_trajectory(
        &mut self,
        source_hash: &str,
        start: &crate::overlay_binding::PyTrajectoryFrame,
        end: &crate::overlay_binding::PyTrajectoryFrame,
        sample_time: Option<f32>,
    ) -> PyResult<()> {
        let binding = molgfx::TrajectoryBinding::new(
            molgfx::schema::DataSource::new(source_hash),
            start.0.clone(),
            end.0.clone(),
        );
        let binding = match sample_time {
            Some(sample_time) => binding.sample_seconds(sample_time),
            None => binding,
        };
        self.inner.bind_trajectory(binding).map_err(error)
    }

    /// Advances a structure's presentation time inside its resident interval.
    #[pyo3(signature = (*, structure, seconds))]
    fn set_trajectory_time(&mut self, structure: &Bound<'_, PyAny>, seconds: f32) -> PyResult<()> {
        self.inner
            .set_trajectory_time(
                molgfx::StructureId::new(crate::id_binding::structure_id(structure)?),
                seconds,
            )
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

    /// Materializes compact browser sources once per molecular provider.
    fn browser_sources<'py>(
        &mut self,
        py: Python<'py>,
    ) -> PyResult<Vec<(u64, &str, Bound<'py, PyBytes>)>> {
        for source in &mut self.browser_sources {
            if source.encoded.get().is_none() {
                // Encoding is fallible and depends on this binding's provider,
                // so it is stored once here rather than in a static initializer.
                let bytes = source.provider.browser_bytes()?;
                let _ = source.encoded.set(bytes);
            }
        }
        Ok(self
            .browser_sources
            .iter()
            .filter_map(|source| {
                source.encoded.get().map(|bytes| {
                    (
                        source.identity,
                        source.name.as_str(),
                        PyBytes::new(py, bytes),
                    )
                })
            })
            .collect())
    }

    /// Adds another molecular source and announces it to subscribers.
    ///
    /// The payload travels to the browser through `browser_sources`; the patch
    /// stream carries only the portable structure descriptor, exactly like the
    /// representation path above. Subscribers are called with no borrow held on
    /// this object, because a viewer answering the announcement reads the new
    /// payload back through `browser_sources` re-entrantly.
    fn add_structure(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        structure: &Bound<'_, PyAny>,
    ) -> PyResult<crate::id_binding::PyStructureId> {
        let (identity, patch) = {
            let mut this = slf.borrow_mut();
            if this.pending.is_some() {
                return Err(PyValueError::new_err(
                    "structures cannot be added inside a scene transaction",
                ));
            }
            let provider = Arc::new(crate::native_adapter::NativeProvider::import(structure)?);
            let source = crate::native_adapter::source(&provider);
            let base_revision = this.inner.revision();
            let identity = this.inner.add_source(&source).map_err(error)?;
            let announced = this
                .inner
                .spec()
                .structures
                .get(&identity)
                .cloned()
                .ok_or_else(|| PyValueError::new_err("added structure is unavailable"))?;
            let ordinal = this.browser_sources.len().checked_add(1).ok_or_else(|| {
                PyValueError::new_err("structure transport columns are exhausted")
            })?;
            this.browser_sources.push(BrowserSource {
                identity: identity.get(),
                name: format!("structure-{ordinal}.bcif"),
                provider,
                encoded: Arc::new(OnceLock::new()),
            });
            (
                identity,
                molgfx::ScenePatch {
                    base_revision,
                    operations: vec![molgfx::schema::PatchOperation::AddStructure {
                        id: identity,
                        source: announced,
                    }],
                },
            )
        };
        deliver(slf, py, &patch)?;
        Ok(crate::id_binding::PyStructureId(identity.get()))
    }

    /// Registers a weak callable receiving committed scene patches as JSON.
    fn subscribe(&mut self, subscriber: Py<PyAny>) {
        self.subscribers.push(subscriber);
    }

    /// Detaches one weak subscription without waiting for another scene edit.
    fn unsubscribe(&mut self, py: Python<'_>, subscriber: &Bound<'_, PyAny>) {
        self.subscribers
            .retain(|reference| !reference.bind(py).is(subscriber));
    }
}
