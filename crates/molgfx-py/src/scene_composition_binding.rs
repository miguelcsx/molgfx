//! Composition entry points on the Python scene: default forms, the pocket
//! view, and placed copies of a structure.

use crate::binding::{error, selection};
use crate::scene_binding::{PyScene, deliver_nonempty};
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

    /// Shares the copied structure's transport source with each new copy and
    /// describes the copies as `AddStructure` operations.
    fn announce_copies(
        &mut self,
        of: u64,
        ids: &[molgfx::StructureId],
    ) -> PyResult<Vec<molgfx::schema::PatchOperation>> {
        let mut operations = Vec::with_capacity(ids.len());
        for id in ids {
            self.share_browser_source(of, id.get())?;
            let source = self
                .inner
                .spec()
                .structures
                .get(id)
                .cloned()
                .ok_or_else(|| PyValueError::new_err("a placed copy is unavailable"))?;
            operations.push(molgfx::schema::PatchOperation::AddStructure { id: *id, source });
        }
        Ok(operations)
    }

    /// Describes inserted representations and extra operations as one patch.
    /// The caller publishes it after releasing the scene borrow.
    pub(super) fn describe_added(
        &self,
        py: Python<'_>,
        base_revision: u64,
        inserted: Vec<molgfx::RepresentationId>,
        extra: Vec<molgfx::schema::PatchOperation>,
    ) -> PyResult<(Vec<Py<PyAny>>, molgfx::ScenePatch)> {
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
        let ids = ids
            .into_iter()
            .map(|id| id.into_python(py))
            .collect::<PyResult<Vec<_>>>()?;
        Ok((
            ids,
            molgfx::ScenePatch {
                base_revision,
                operations,
            },
        ))
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
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        instances: &Bound<'_, PyAny>,
        of: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<PyAssemblyCopy>> {
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
        let of = of.map(crate::id_binding::structure_id).transpose()?;
        let (patch, described) = {
            let mut this = slf.borrow_mut();
            this.reject_pending("an assembly cannot be added inside a scene transaction")?;
            let target = match of {
                Some(of) => of,
                None => this.structure_id,
            };
            let base_revision = this.inner.revision();
            let matrices: Vec<[f32; 16]> = parsed.iter().map(|(matrix, _)| *matrix).collect();
            let ids = this
                .inner
                .place_many(molgfx::StructureId::new(target), &matrices)
                .map_err(error)?;
            let operations = this.announce_copies(target, &ids)?;
            let described = ids
                .iter()
                .zip(parsed)
                .map(|(id, (_, chains))| PyAssemblyCopy {
                    structure: id.get(),
                    selection: molgfx::chain_selection(&chains).source().to_owned(),
                    chains,
                })
                .collect::<Vec<_>>();
            (
                molgfx::ScenePatch {
                    base_revision,
                    operations,
                },
                described,
            )
        };
        deliver_nonempty(slf, py, &patch)?;
        Ok(described)
    }

    /// Adds the default representations for a structure's molecule types
    /// (cartoon for polymers, ball-and-stick for ligands, and so on) and
    /// returns their ids.
    #[pyo3(signature = (*, structure=None))]
    fn auto(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        structure: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let (ids, patch) = {
            let mut this = slf.borrow_mut();
            this.reject_pending("default forms cannot be added inside a scene transaction")?;
            let target = match structure {
                Some(structure) => crate::id_binding::structure_id(structure)?,
                None => this.structure_id,
            };
            let base_revision = this.inner.revision();
            let inserted = this
                .inner
                .add_auto(molgfx::StructureId::new(target))
                .map_err(error)?;
            this.describe_added(py, base_revision, inserted, Vec::new())
        }?;
        deliver_nonempty(slf, py, &patch)?;
        Ok(ids)
    }

    /// Adds a binding-pocket view around `focus` (a query string or `sel`
    /// query) and returns the representation ids.
    ///
    /// Raises `SpecError` when `focus` selects no atoms, for example
    /// `"resname LIG"` on an entry that has no ligand.
    #[pyo3(signature = (focus, *, structure=None, style=None))]
    fn pocket(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        focus: &Bound<'_, PyAny>,
        structure: Option<&Bound<'_, PyAny>>,
        style: Option<PyRef<'_, crate::pocket_binding::PyPocketStyle>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let (ids, patch) = {
            let mut this = slf.borrow_mut();
            this.reject_pending("a pocket view cannot be added inside a scene transaction")?;
            let target = match structure {
                Some(structure) => crate::id_binding::structure_id(structure)?,
                None => this.structure_id,
            };
            let focus = molgfx::Selection::from(selection(focus)?);
            let style = style.map_or_else(molgfx::preset::PocketStyle::default, |style| style.0);
            let base_revision = this.inner.revision();
            let inserted = this
                .inner
                .add_pocket(molgfx::StructureId::new(target), focus.clone(), style)
                .map_err(error)?;
            let focus = molgfx::schema::PatchOperation::SetFocus {
                selection: Some(focus),
            };
            this.describe_added(py, base_revision, inserted, vec![focus])
        }?;
        deliver_nonempty(slf, py, &patch)?;
        Ok(ids)
    }

    /// A copy of a structure's molecular source at a column-major 4x4 affine
    /// matrix (16 floats), or at `rotation` (3x3, `x' = R @ x + t`) and
    /// `translation` (3 floats); it is a structure of its own with its own
    /// representations.
    ///
    /// The matrix is column-major: a row-major `NumPy` `M` must be passed as
    /// `M.T.reshape(-1).tolist()`. `rotation=`/`translation=` take the row-major
    /// `R` and `t` of a molframe `superpose` result directly, with no
    /// conversion. `molgfx.place.apply(scene, superposition)` does the same for
    /// a whole superposition. For many copies use `place_many`, which is one
    /// revision and one patch.
    #[pyo3(signature = (matrix=None, *, rotation=None, translation=None, structure=None))]
    pub(super) fn place(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        matrix: Option<Vec<f32>>,
        rotation: Option<[[f32; 3]; 3]>,
        translation: Option<[f32; 3]>,
        structure: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let matrix = placement_matrix(matrix, rotation, translation)?;
        let ids = place_copies(slf, py, structure, &[matrix])?;
        Ok(Py::new(py, crate::id_binding::PyStructureId(ids[0]))?.into_any())
    }

    /// Copies of a structure at each of `matrices` (16 column-major floats
    /// each), as one revision and one patch; the cost is linear in the number
    /// of copies and independent of the atom count. Returns the new ids.
    #[pyo3(signature = (matrices, *, structure=None))]
    fn place_many(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        matrices: &Bound<'_, PyAny>,
        structure: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let parsed = matrices
            .try_iter()?
            .map(|item| {
                let values: Vec<f32> = item?.extract()?;
                placement_matrix(Some(values), None, None)
            })
            .collect::<PyResult<Vec<_>>>()?;
        place_copies(slf, py, structure, &parsed)?
            .into_iter()
            .map(|id| Ok(Py::new(py, crate::id_binding::PyStructureId(id))?.into_any()))
            .collect()
    }
}

/// The column-major matrix of a placement given as a matrix or as a rigid
/// transform.
fn placement_matrix(
    matrix: Option<Vec<f32>>,
    rotation: Option<[[f32; 3]; 3]>,
    translation: Option<[f32; 3]>,
) -> PyResult<[f32; 16]> {
    match (matrix, rotation, translation) {
        (Some(matrix), None, None) => matrix
            .try_into()
            .map_err(|_| PyValueError::new_err("matrix needs 16 values (column-major 4x4)")),
        (None, rotation, translation) if rotation.is_some() || translation.is_some() => {
            const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
            let (rotation, translation) = match (rotation, translation) {
                (Some(rotation), Some(translation)) => (rotation, translation),
                (Some(rotation), None) => (rotation, [0.0; 3]),
                (None, Some(translation)) => (IDENTITY, translation),
                (None, None) => (IDENTITY, [0.0; 3]),
            };
            Ok(molgfx::placement::rigid(rotation, translation))
        }
        (None, None, None) => Err(PyValueError::new_err(
            "place needs a 16-value matrix, or rotation= (3x3) and translation= (3)",
        )),
        _ => Err(PyValueError::new_err(
            "pass either a matrix or rotation=/translation=, not both",
        )),
    }
}

/// Places one copy per matrix and delivers them as a single patch.
fn place_copies(
    slf: &Bound<'_, PyScene>,
    py: Python<'_>,
    structure: Option<&Bound<'_, PyAny>>,
    matrices: &[[f32; 16]],
) -> PyResult<Vec<u64>> {
    let of = structure.map(crate::id_binding::structure_id).transpose()?;
    let (ids, patch) = {
        let mut this = slf.borrow_mut();
        this.reject_pending("a placement cannot be added inside a scene transaction")?;
        let of = match of {
            Some(of) => of,
            None => this.structure_id,
        };
        let base_revision = this.inner.revision();
        let ids = this
            .inner
            .place_many(molgfx::StructureId::new(of), matrices)
            .map_err(error)?;
        let operations = this.announce_copies(of, &ids)?;
        (
            ids.iter().map(|id| id.get()).collect::<Vec<_>>(),
            molgfx::ScenePatch {
                base_revision,
                operations,
            },
        )
    };
    deliver_nonempty(slf, py, &patch)?;
    Ok(ids)
}
