//! Mutable Python scene and browser-source transport.

use crate::binding::{PyRepresentation, PyScenePatch, PySceneSpec, error, selection};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyBytes};
use std::sync::{Arc, OnceLock};

/// A mutable, declarative scene over one or more `molframe` structures.
///
/// Build it from a structure (`Scene(structure)`), add representations and
/// overlays with `add`, then draw it with a `Renderer`. Every edit is a patch
/// against `revision`, so a scene can be observed, replayed and serialized with
/// `to_json`. `Scene()` makes an empty scene that holds only overlays and
/// volumes until `add_structure` supplies a molecule.
#[pyclass(name = "Scene")]
pub(super) struct PyScene {
    pub(super) inner: molgfx::Scene,
    pub(super) pending: Option<Vec<molgfx::schema::PatchOperation>>,
    pub(super) structure_id: u64,
    pub(super) browser_sources: Vec<BrowserSource>,
    pub(super) browser_bindings: crate::bulk_binding::BindingCache,
    pub(super) subscribers: Vec<Py<PyAny>>,
    pub(super) runtime_subscribers: Vec<Py<PyAny>>,
    pub(super) browser_frames: crate::motion_binding::FrameCache,
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
}

/// Refuses a representation that would select no atom, naming the query.
fn reject_empty_target(
    scene: &molgfx::Scene,
    representation: &molgfx::RepresentationSpec,
) -> PyResult<()> {
    if scene.spec().structures.is_empty() {
        return Ok(());
    }
    let query = representation.selection();
    if scene.selection_bounds(query).map_err(error)?.is_some() {
        return Ok(());
    }
    Err(error(molgfx::Error::InvalidSpec(format!(
        "the representation target {query:?} selects no atoms in this scene; check the query \
         (`chain` and `resid` use the author's names, `label_chain` and `label_resid` the mmCIF \
         label ones, and `sel.residue(...)` takes a residue name) or pass `allow_empty=True` to \
         add it anyway"
    ))))
}

/// Stages one transaction operation or publishes it after releasing the scene borrow.
pub(super) fn stage_or_apply(
    slf: &Bound<'_, PyScene>,
    py: Python<'_>,
    operation: molgfx::schema::PatchOperation,
) -> PyResult<()> {
    let patch = {
        let mut scene = slf.borrow_mut();
        if let Some(pending) = &mut scene.pending {
            pending.push(operation);
            return Ok(());
        }
        let patch = molgfx::ScenePatch {
            base_revision: scene.inner.revision(),
            operations: vec![operation],
        };
        scene.inner.apply(&patch).map_err(error)?;
        patch
    };
    deliver(slf, py, &patch)
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
    crate::subscription_binding::publish(slf, py, &encoded, false)
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
                browser_bindings: crate::bulk_binding::BindingCache::default(),
                subscribers: Vec::new(),
                runtime_subscribers: Vec::new(),
                browser_frames: crate::motion_binding::FrameCache::default(),
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
            browser_bindings: crate::bulk_binding::BindingCache::default(),
            subscribers: Vec::new(),
            runtime_subscribers: Vec::new(),
            browser_frames: crate::motion_binding::FrameCache::default(),
        })
    }

    fn __repr__(&self) -> String {
        let spec = self.inner.spec();
        format!(
            "Scene(structures={}, representations={}, revision={})",
            spec.structures.len(),
            spec.representations.len(),
            self.inner.revision(),
        )
    }

    fn unresolved_overlays(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.unresolved_overlays())
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    /// Adds a representation, volume, segmentation, label, measurement,
    /// interaction, trajectory descriptor or ellipsoid, and returns its id.
    ///
    /// A representation whose `target` selects no atom of the scene raises
    /// `SpecError`, because it would draw nothing and a typo in a query would
    /// otherwise go unnoticed (`Scene.pocket` rejects an empty focus the same
    /// way). The message names the query and the usual causes. Pass
    /// `allow_empty=True` to add it anyway, for example a ligand layer that is
    /// declared before the ligand is loaded. The check is skipped for a scene
    /// that holds no structure yet.
    #[pyo3(signature = (item, *, allow_empty=false))]
    fn add(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        item: &Bound<'_, PyAny>,
        allow_empty: bool,
    ) -> PyResult<Py<PyAny>> {
        let (id, patch) = {
            let mut this = slf.borrow_mut();
            if this.pending.is_some() {
                return Err(PyValueError::new_err(
                    "scene items cannot be added inside a scene transaction",
                ));
            }
            let base_revision = this.inner.revision();
            let (id, operation) =
                if let Ok(representation) = item.extract::<PyRef<'_, PyRepresentation>>() {
                    if !allow_empty {
                        reject_empty_target(&this.inner, &representation.0)?;
                    }
                    let id = representation.add_to(&mut this.inner).map_err(error)?;
                    let inserted = this
                        .inner
                        .spec()
                        .representations
                        .get(&id)
                        .cloned()
                        .ok_or_else(|| {
                            PyValueError::new_err("inserted representation is unavailable")
                        })?;
                    (
                        crate::id_binding::PySceneId::Representation(id.get()),
                        molgfx::schema::PatchOperation::AddRepresentation {
                            id,
                            representation: inserted,
                        },
                    )
                } else {
                    crate::overlay_binding::add_item(item, &mut this.inner)?
                };
            (
                id,
                molgfx::ScenePatch {
                    base_revision,
                    operations: vec![operation],
                },
            )
        };
        deliver(slf, py, &patch)?;
        id.into_python(py)
    }

    /// Binds one scalar per atom to a structure and returns a `ScalarProperty`.
    ///
    /// `source_hash` identifies the data (any stable string, such as a content
    /// hash); `values` must have one entry per atom. Colour with
    /// `molgfx.color.property(property=..., ramp=..., domain=...)`, or use
    /// `molgfx.by_array(scene, values, ...)`, which derives the hash from the data.
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
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        representation: &Bound<'_, PyAny>,
        visible: bool,
    ) -> PyResult<()> {
        stage_or_apply(
            slf,
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
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        representation: &Bound<'_, PyAny>,
        opacity: f32,
    ) -> PyResult<()> {
        stage_or_apply(
            slf,
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
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        representation: &Bound<'_, PyAny>,
        visual: Option<&crate::visual_binding::PyVisualStyle>,
    ) -> PyResult<()> {
        stage_or_apply(
            slf,
            py,
            molgfx::schema::PatchOperation::SetVisual {
                id: molgfx::RepresentationId::new(crate::id_binding::representation_id(
                    representation,
                )?),
                visual: visual.map(|value| value.0.clone()),
            },
        )
    }

    fn focus(slf: &Bound<'_, Self>, py: Python<'_>, target: &Bound<'_, PyAny>) -> PyResult<()> {
        stage_or_apply(
            slf,
            py,
            molgfx::schema::PatchOperation::SetFocus {
                selection: Some(selection(target)?.into()),
            },
        )
    }

    fn apply(slf: &Bound<'_, Self>, py: Python<'_>, patch: &PyScenePatch) -> PyResult<()> {
        {
            let mut scene = slf.borrow_mut();
            if scene.pending.is_some() {
                return Err(PyValueError::new_err(
                    "patches cannot be applied inside a scene transaction",
                ));
            }
            scene.inner.apply(&patch.0).map_err(error)?;
        }
        deliver(slf, py, &patch.0)
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
