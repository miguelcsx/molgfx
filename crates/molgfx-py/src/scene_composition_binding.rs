//! Composition entry points on the Python scene: default forms, the pocket
//! view, and placed copies of a structure.

use crate::binding::{error, selection};
use crate::scene_binding::PyScene;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

impl PyScene {
    /// Gives a placed copy the transport payload of the structure it copies,
    /// encoded once and shared, so a viewer can bind it without a second
    /// serialization.
    fn share_browser_source(&mut self, of: u64, copy: u64) -> PyResult<()> {
        let source = self
            .browser_sources
            .iter()
            .find(|source| source.identity == of)
            .map(|source| {
                (
                    source.name.clone(),
                    source.provider.clone(),
                    std::sync::Arc::clone(&source.encoded),
                )
            })
            .ok_or_else(|| PyValueError::new_err("the copied structure has no transport source"))?;
        self.browser_sources
            .push(crate::scene_binding::BrowserSource {
                identity: copy,
                name: format!("placed-{copy}-{}", source.0),
                provider: source.1,
                encoded: source.2,
            });
        Ok(())
    }

    /// Publishes inserted representations, then any extra operations, as one
    /// patch, and returns the new identities.
    fn publish_added(
        &mut self,
        py: Python<'_>,
        base_revision: u64,
        inserted: Vec<molgfx::RepresentationId>,
        extra: Vec<molgfx::schema::PatchOperation>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let mut operations = Vec::with_capacity(inserted.len() + extra.len());
        let mut ids = Vec::with_capacity(inserted.len());
        for id in inserted {
            let representation = self
                .inner
                .spec()
                .representations
                .get(&id)
                .cloned()
                .ok_or_else(|| PyValueError::new_err("inserted representation is unavailable"))?;
            operations
                .push(molgfx::schema::PatchOperation::AddRepresentation { id, representation });
            ids.push(crate::id_binding::PySceneId::Representation(id.get()));
        }
        operations.extend(extra);
        if !operations.is_empty() {
            self.publish(
                py,
                &molgfx::ScenePatch {
                    base_revision,
                    operations,
                },
            )?;
        }
        ids.into_iter()
            .map(|id| id.into_python(py))
            .collect::<PyResult<Vec<_>>>()
    }
}

/// One transform of a biological assembly and the chains it applies to.
#[derive(Clone, Debug)]
#[pyclass(name = "AssemblyCopy", frozen, skip_from_py_object)]
pub(super) struct PyAssemblyCopy {
    structure: u64,
    chains: Vec<String>,
    selection: String,
}

#[pymethods]
impl PyAssemblyCopy {
    /// The placed structure holding this copy.
    #[getter]
    fn structure(&self) -> crate::id_binding::PyStructureId {
        crate::id_binding::PyStructureId(self.structure)
    }

    /// `label_asym_id` of each chain the transform applies to.
    #[getter]
    fn chains(&self) -> Vec<String> {
        self.chains.clone()
    }

    /// A query selecting exactly those chains.
    #[getter]
    fn selection(&self) -> &str {
        &self.selection
    }

    fn __repr__(&self) -> String {
        format!("AssemblyCopy(chains={})", self.chains.join(","))
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAssemblyCopy>()
}

#[pymethods]
impl PyScene {
    /// Places one copy of the structure per assembly instance.
    ///
    /// Each item has a `matrix` (16 column-major floats) and `chains`
    /// (`label_asym_id` values), as `molframe.crystal.assembly` returns. Draw
    /// each returned copy with its own `selection` and `.on(copy.structure)`;
    /// the identity transform comes back like any other, so the copies replace
    /// the deposited asymmetric unit rather than add to it.
    #[pyo3(signature = (instances, *, of=None))]
    fn assembly(
        &mut self,
        py: Python<'_>,
        instances: &Bound<'_, PyAny>,
        of: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<PyAssemblyCopy>> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "an assembly cannot be added inside a scene transaction",
            ));
        }
        let target = match of {
            Some(of) => crate::id_binding::structure_id(of)?,
            None => self.structure_id,
        };
        let parsed = instances
            .try_iter()?
            .map(|instance| {
                let instance = instance?;
                let matrix: Vec<f32> = instance.getattr("matrix")?.extract()?;
                let matrix: [f32; 16] = matrix.try_into().map_err(|_| {
                    PyValueError::new_err("an assembly matrix needs 16 values (column-major 4x4)")
                })?;
                let chains: Vec<String> = instance.getattr("chains")?.extract()?;
                Ok((matrix, chains))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let base_revision = self.inner.revision();
        let mut operations = Vec::with_capacity(parsed.len());
        let mut described = Vec::with_capacity(parsed.len());
        for (matrix, chains) in parsed {
            let id = self
                .inner
                .place(molgfx::StructureId::new(target), matrix)
                .map_err(error)?;
            self.share_browser_source(target, id.get())?;
            let announced = self
                .inner
                .spec()
                .structures
                .get(&id)
                .cloned()
                .ok_or_else(|| PyValueError::new_err("a placed copy is unavailable"))?;
            operations.push(molgfx::schema::PatchOperation::AddStructure {
                id,
                source: announced,
            });
            described.push(PyAssemblyCopy {
                structure: id.get(),
                selection: molgfx::chain_selection(&chains).source().to_owned(),
                chains,
            });
        }
        if !operations.is_empty() {
            self.publish(
                py,
                &molgfx::ScenePatch {
                    base_revision,
                    operations,
                },
            )?;
        }
        Ok(described)
    }

    #[pyo3(signature = (*, structure=None))]
    fn auto(
        &mut self,
        py: Python<'_>,
        structure: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "default forms cannot be added inside a scene transaction",
            ));
        }
        let target = match structure {
            Some(structure) => crate::id_binding::structure_id(structure)?,
            None => self.structure_id,
        };
        let base_revision = self.inner.revision();
        let inserted = self
            .inner
            .add_auto(molgfx::StructureId::new(target))
            .map_err(error)?;
        self.publish_added(py, base_revision, inserted, Vec::new())
    }

    #[pyo3(signature = (focus, *, structure=None, style=None))]
    fn pocket(
        &mut self,
        py: Python<'_>,
        focus: &Bound<'_, PyAny>,
        structure: Option<&Bound<'_, PyAny>>,
        style: Option<PyRef<'_, crate::pocket_binding::PyPocketStyle>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "a pocket view cannot be added inside a scene transaction",
            ));
        }
        let target = match structure {
            Some(structure) => crate::id_binding::structure_id(structure)?,
            None => self.structure_id,
        };
        let focus = molgfx::Selection::from(selection(focus)?);
        let style = style.map_or_else(molgfx::preset::PocketStyle::default, |style| style.0);
        let base_revision = self.inner.revision();
        let inserted = self
            .inner
            .add_pocket(molgfx::StructureId::new(target), focus.clone(), style)
            .map_err(error)?;
        let focus = molgfx::schema::PatchOperation::SetFocus {
            selection: Some(focus),
        };
        self.publish_added(py, base_revision, inserted, vec![focus])
    }

    /// A copy of a structure's molecular source at a column-major 4×4 affine
    /// matrix; it is a structure of its own with its own representations.
    #[pyo3(signature = (matrix, *, structure=None))]
    fn place(
        &mut self,
        py: Python<'_>,
        matrix: Vec<f32>,
        structure: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        if self.pending.is_some() {
            return Err(PyValueError::new_err(
                "a placement cannot be added inside a scene transaction",
            ));
        }
        let matrix: [f32; 16] = matrix
            .try_into()
            .map_err(|_| PyValueError::new_err("matrix needs 16 values (column-major 4x4)"))?;
        let of = match structure {
            Some(structure) => crate::id_binding::structure_id(structure)?,
            None => self.structure_id,
        };
        let base_revision = self.inner.revision();
        let id = self
            .inner
            .place(molgfx::StructureId::new(of), matrix)
            .map_err(error)?;
        self.share_browser_source(of, id.get())?;
        let source = self
            .inner
            .spec()
            .structures
            .get(&id)
            .cloned()
            .ok_or_else(|| PyValueError::new_err("the placed structure is unavailable"))?;
        self.publish(
            py,
            &molgfx::ScenePatch {
                base_revision,
                operations: vec![molgfx::schema::PatchOperation::AddStructure { id, source }],
            },
        )?;
        Ok(Py::new(py, crate::id_binding::PyStructureId(id.get()))?.into_any())
    }
}
