//! Cached binary transport of immutable scientific assets; encoding costs O(asset size) once.

use crate::scene_binding::PyScene;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyBytes};
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Column = Py<PyBytes>;
type Volume = (u64, Column);
type HydrogenBonds = (u64, Column, Option<f32>, String);
type Surface = (u64, String, Column, Column, Column, Column);
pub(super) type BrowserBindings = (Vec<Volume>, Vec<HydrogenBonds>, Vec<Surface>);

#[derive(Default)]
pub(super) struct BindingCache {
    columns: BTreeMap<(u8, u64), (Box<str>, Vec<Column>)>,
}

impl BindingCache {
    fn get(
        &mut self,
        key: (u8, u64),
        source: &str,
        encode: impl FnOnce() -> PyResult<Vec<Column>>,
    ) -> PyResult<&[Column]> {
        if self
            .columns
            .get(&key)
            .is_none_or(|(previous, _)| &**previous != source)
        {
            let _ = self.columns.insert(key, (source.into(), encode()?));
        }
        self.columns
            .get(&key)
            .map(|(_, columns)| &columns[..])
            .ok_or_else(|| PyValueError::new_err("scientific transport cache is unavailable"))
    }
}

#[pymethods]
impl PyScene {
    /// Returns cached little-endian bulk columns for the canonical browser host.
    /// Surface metadata carries method, controls, discarded face count and the optional original resident-frame interval;
    /// its four columns are vertices, normals, faces and original atom rows.
    pub(super) fn browser_bindings(&mut self, py: Python<'_>) -> PyResult<BrowserBindings> {
        export(
            py,
            &self.inner,
            &mut self.browser_bindings,
            &mut self.browser_frames,
        )
    }
}

fn export(
    py: Python<'_>,
    scene: &molgfx::Scene,
    cache: &mut BindingCache,
    frames: &mut crate::motion_binding::FrameCache,
) -> PyResult<BrowserBindings> {
    let mut seen = BTreeSet::new();
    let mut volumes = Vec::new();
    for (id, binding) in scene.bound_volumes() {
        let key = (0, id.get());
        seen.insert(key);
        let columns = cache.get(key, &binding.source().content_hash, || {
            Ok(vec![words(
                py,
                binding.values().len(),
                binding.values().iter().map(|value| value.to_bits()),
            )?])
        })?;
        let [column] = columns else {
            return Err(incomplete());
        };
        volumes.push((id.get(), column.clone_ref(py)));
    }
    let mut hydrogen_bonds = Vec::new();
    for (id, binding) in scene.bound_hydrogen_bonds() {
        let key = (1, id.get());
        seen.insert(key);
        let columns = cache.get(key, &binding.source().content_hash, || {
            let count = binding
                .triples()
                .count()
                .checked_mul(3)
                .ok_or_else(overflow)?;
            Ok(vec![words(py, count, binding.triples().flatten())?])
        })?;
        let [column] = columns else {
            return Err(incomplete());
        };
        let interval = binding
            .pose()
            .map(|pose| frames.interval(py, pose))
            .transpose()?
            .flatten();
        let metadata = serde_json::to_string(&interval)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        hydrogen_bonds.push((
            id.get(),
            column.clone_ref(py),
            binding.candidate_distance(),
            metadata,
        ));
    }
    let mut surfaces = Vec::new();
    for (id, binding) in scene.bound_molecular_surfaces() {
        let key = (2, id.get());
        seen.insert(key);
        let mesh = binding.geometry().mesh();
        let columns = cache.get(key, &binding.source().content_hash, || {
            Ok(vec![
                float_triples(py, &mesh.geometry.vertices)?,
                float_triples(py, &mesh.geometry.vertex_normals)?,
                words(
                    py,
                    mesh.geometry
                        .faces
                        .len()
                        .checked_mul(3)
                        .ok_or_else(overflow)?,
                    mesh.geometry.faces.iter().flat_map(|face| face.0),
                )?,
                words(py, mesh.atom_rows.len(), mesh.atom_rows.iter().copied())?,
            ])
        })?;
        let [vertices, normals, faces, atoms] = columns else {
            return Err(incomplete());
        };
        let metadata = serde_json::to_string(&(
            binding.method(),
            binding.controls(),
            mesh.discarded_degenerate_faces,
            match binding.geometry().pose() {
                Some(pose) => frames.interval(py, pose)?,
                None => None,
            },
        ))
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        surfaces.push((
            id.get(),
            metadata,
            vertices.clone_ref(py),
            normals.clone_ref(py),
            faces.clone_ref(py),
            atoms.clone_ref(py),
        ));
    }
    cache.columns.retain(|key, _| seen.contains(key));
    Ok((volumes, hydrogen_bonds, surfaces))
}

pub(super) fn float_triples(py: Python<'_>, values: &[[f32; 3]]) -> PyResult<Column> {
    words(
        py,
        values.len().checked_mul(3).ok_or_else(overflow)?,
        values.iter().flatten().map(|value| value.to_bits()),
    )
}

fn words(py: Python<'_>, count: usize, values: impl Iterator<Item = u32>) -> PyResult<Column> {
    let bytes = count.checked_mul(4).ok_or_else(overflow)?;
    Ok(PyBytes::new_with(py, bytes, |output| {
        let mut written = 0;
        for (destination, word) in output.as_chunks_mut::<4>().0.iter_mut().zip(values) {
            destination.copy_from_slice(&word.to_le_bytes());
            written += 1;
        }
        if written != count {
            return Err(incomplete());
        }
        Ok(())
    })?
    .unbind())
}

fn overflow() -> PyErr {
    PyValueError::new_err("scientific transport column exceeds addressable storage")
}

fn incomplete() -> PyErr {
    PyValueError::new_err("scientific transport columns are incomplete")
}
