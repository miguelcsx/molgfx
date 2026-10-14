//! One-call browser transport: every column the notebook widget syncs, built
//! here so the Python layer only forwards values.
//!
//! Payload columns are cached `bytes` objects shared by reference, so a call
//! copies no payload. Coordinate frames are the one place a copy remains: a
//! frame's positions leave the engine as one little-endian `bytes` object,
//! built once and cached per frame. Exposing the borrowed `[[f32; 3]]` column
//! without that copy needs a buffer-protocol export or `numpy` borrowing, and
//! both require `unsafe`, which this workspace forbids outside audited
//! `bytemuck` casts; the single cached copy is the safe minimum.

use crate::scene_binding::PyScene;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyBytes};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
};

type Column = Py<PyBytes>;
type Interval = (String, String, String, f64);

/// Folds the identity of a cached column (its address) into a fingerprint.
fn fold(state: &mut DefaultHasher, column: &Column) {
    (column.as_ptr() as usize).hash(state);
}

/// Every synced column of one scene, plus fingerprints of its heavy assets.
///
/// The fingerprints change exactly when a bound asset or resident interval is
/// replaced; style edits leave them equal, so large buffers are never compared.
#[pyclass(name = "BrowserTransport", frozen, get_all)]
pub(super) struct PyBrowserTransport {
    /// Structure identities in declaration order.
    structure_ids: Vec<u64>,
    /// Transport file names, one per structure.
    structure_names: Vec<String>,
    /// `BinaryCIF` payloads; a structure sharing another's bytes carries `b""`.
    structure_payloads: Vec<Column>,
    /// The structure whose payload each structure reads.
    structure_sources: Vec<u64>,
    /// Volume identity and little-endian scalar words.
    volumes: Vec<(String, Column)>,
    /// Identity, bond rows, candidate distance and interval metadata.
    hydrogen_bonds: Vec<(String, Column, Option<f32>, String)>,
    /// Identity, metadata, vertices, normals, faces and atom rows.
    surfaces: Vec<(String, String, Column, Column, Column, Column)>,
    /// Frame identity, frame index, seconds and little-endian positions.
    frames: Vec<(String, String, f64, Column)>,
    /// Bound trajectory intervals: identity, start, end and sample seconds.
    trajectories: Vec<Interval>,
    /// Imperative playback intervals: identity, start, end and sample seconds.
    playback: Vec<Interval>,
    /// Identity of the bound scientific assets.
    binding_fingerprint: u64,
    /// Identity of the resident coordinate frames and intervals.
    motion_fingerprint: u64,
}

#[pymethods]
impl PyBrowserTransport {
    /// The playback interval of one structure, named by its decimal identity.
    fn playback_for(&self, structure: &str) -> PyResult<Interval> {
        self.playback
            .iter()
            .find(|interval| interval.0 == structure)
            .cloned()
            .ok_or_else(|| PyValueError::new_err("the structure has no playback interval"))
    }
}

#[pymethods]
impl PyScene {
    /// Builds every browser column in one call.
    fn browser_transport(&mut self, py: Python<'_>) -> PyResult<PyBrowserTransport> {
        self.encode_sources(py)?;
        let (volumes, hydrogen_bonds, surfaces) = self.browser_bindings(py)?;
        let (frames, trajectories) = self.browser_motion_bindings(py)?;
        let playback = self.browser_playback_bindings(py)?;

        let mut binding = DefaultHasher::new();
        for (id, column) in &volumes {
            id.hash(&mut binding);
            fold(&mut binding, column);
        }
        for (id, rows, distance, metadata) in &hydrogen_bonds {
            id.hash(&mut binding);
            fold(&mut binding, rows);
            distance.map(f32::to_bits).hash(&mut binding);
            metadata.hash(&mut binding);
        }
        for (id, metadata, vertices, normals, faces, atoms) in &surfaces {
            id.hash(&mut binding);
            metadata.hash(&mut binding);
            for column in [vertices, normals, faces, atoms] {
                fold(&mut binding, column);
            }
        }
        let mut motion = DefaultHasher::new();
        for (id, index, seconds, column) in &frames {
            (id, index, seconds.to_bits()).hash(&mut motion);
            fold(&mut motion, column);
        }
        for (id, start, end, _) in trajectories.iter().chain(&playback) {
            (id, start, end).hash(&mut motion);
        }

        let identity = |id: u64| id.to_string();
        let intervals = |rows: Vec<(u64, u64, u64, f64)>| -> Vec<Interval> {
            rows.into_iter()
                .map(|(id, start, end, seconds)| {
                    (identity(id), identity(start), identity(end), seconds)
                })
                .collect()
        };
        let (ids, names, payloads, sources) = structure_columns(self, py)?;
        Ok(PyBrowserTransport {
            structure_ids: ids,
            structure_names: names,
            structure_payloads: payloads,
            structure_sources: sources,
            volumes: volumes
                .into_iter()
                .map(|(id, values)| (identity(id), values))
                .collect(),
            hydrogen_bonds: hydrogen_bonds
                .into_iter()
                .map(|(id, rows, distance, metadata)| (identity(id), rows, distance, metadata))
                .collect(),
            surfaces: surfaces
                .into_iter()
                .map(|(id, metadata, v, n, f, a)| (identity(id), metadata, v, n, f, a))
                .collect(),
            frames: frames
                .into_iter()
                .map(|(id, index, seconds, positions)| {
                    (identity(id), identity(index), seconds, positions)
                })
                .collect(),
            trajectories: intervals(trajectories),
            playback: intervals(playback),
            binding_fingerprint: binding.finish(),
            motion_fingerprint: motion.finish(),
        })
    }

    /// Whether a committed patch adds a structure and so needs the structure columns re-sent.
    #[staticmethod]
    fn patch_changes_structures(patch_json: &str) -> PyResult<bool> {
        use molgfx::schema::PatchOperation::AddStructure;
        let patch = molgfx::ScenePatch::from_json(patch_json).map_err(crate::binding::error)?;
        Ok(patch
            .operations
            .iter()
            .any(|operation| matches!(operation, AddStructure { .. })))
    }
}

type StructureColumns = (Vec<u64>, Vec<String>, Vec<Column>, Vec<u64>);

/// Structure columns in declaration order; identical payloads travel once.
fn structure_columns(scene: &PyScene, py: Python<'_>) -> PyResult<StructureColumns> {
    let sources = &scene.browser_sources;
    let missing = || PyValueError::new_err("structure payload is not encoded");
    let mut owners: Vec<usize> = Vec::new();
    let mut columns = (
        Vec::with_capacity(sources.len()),
        Vec::with_capacity(sources.len()),
        Vec::with_capacity(sources.len()),
        Vec::with_capacity(sources.len()),
    );
    let empty = PyBytes::new(py, b"").unbind();
    for (index, source) in sources.iter().enumerate() {
        let payload = source.encoded.get().ok_or_else(missing)?;
        let mut owner = index;
        for &candidate in &owners {
            let other = &sources[candidate];
            let other_payload = other.encoded.get().ok_or_else(missing)?;
            if Arc::ptr_eq(&source.encoded, &other.encoded)
                || payload.as_ptr() == other_payload.as_ptr()
                || payload.bind(py).as_bytes() == other_payload.bind(py).as_bytes()
            {
                owner = candidate;
                break;
            }
        }
        if owner == index {
            owners.push(index);
        }
        columns.0.push(source.identity);
        columns.1.push(source.name.clone());
        columns.2.push(if owner == index {
            payload.clone_ref(py)
        } else {
            empty.clone_ref(py)
        });
        columns.3.push(sources[owner].identity);
    }
    Ok(columns)
}
