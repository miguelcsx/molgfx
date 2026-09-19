//! Streaming manifests: composition plus content-addressed payloads.
//!
//! A manifest never carries caller-owned data. It carries the composition and,
//! for every payload it depends on, a semantic kind and a SHA-256 address. The
//! address is derived rather than declared — [`PyContentAddress::digest`] is the
//! only way to produce one — so a reference cannot claim a payload it does not
//! describe.

use super::composition::PySceneDescription;
use crate::error::manifest;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::path::PathBuf;

/// SHA-256 identity of an immutable external payload.
#[pyclass(name = "ContentAddress", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyContentAddress(pub(crate) molgfx::core::ContentAddress);

#[pymethods]
impl PyContentAddress {
    /// Hashes one contiguous payload without retaining a second copy.
    #[staticmethod]
    fn digest(payload: &[u8]) -> Self {
        Self(molgfx::core::ContentAddress::digest(payload))
    }

    /// The digest a store or Merkle node prints and compares: 64 lowercase
    /// hexadecimal digits, in the order the bytes were hashed.
    #[getter]
    fn hex(&self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut hex = String::with_capacity(64);
        for byte in self.0.bytes() {
            hex.push(char::from(DIGITS[usize::from(byte >> 4)]));
            hex.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
        }
        hex
    }

    fn __str__(&self) -> String {
        self.hex()
    }

    fn __repr__(&self) -> String {
        format!("ContentAddress('{}')", self.__str__())
    }
}

/// Semantic class of one caller-owned immutable payload.
#[pyclass(name = "ReferencedPayloadKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyReferencedPayloadKind {
    /// Parsed molecular structure or one paged structure chunk.
    Structure,
    /// Structure-scoped property column.
    Property,
    /// One trajectory frame chunk.
    Frames,
    /// Sparse scalar or categorical volume brick.
    Brick,
    /// Indexed mesh chunk.
    Mesh,
    /// Always-available coarse representation.
    Proxy,
    /// Generic point positions and source keys.
    Points,
    /// Shared analytic template plus rigid transforms and source keys.
    Instances,
    /// One generic typed attribute column.
    Attribute,
    /// Generic spatial relation anchors and source keys.
    Relations,
}

impl From<PyReferencedPayloadKind> for molgfx::core::ReferencedPayloadKind {
    fn from(value: PyReferencedPayloadKind) -> Self {
        match value {
            PyReferencedPayloadKind::Structure => Self::Structure,
            PyReferencedPayloadKind::Property => Self::Property,
            PyReferencedPayloadKind::Frames => Self::Frames,
            PyReferencedPayloadKind::Brick => Self::Brick,
            PyReferencedPayloadKind::Mesh => Self::Mesh,
            PyReferencedPayloadKind::Proxy => Self::Proxy,
            PyReferencedPayloadKind::Points => Self::Points,
            PyReferencedPayloadKind::Instances => Self::Instances,
            PyReferencedPayloadKind::Attribute => Self::Attribute,
            PyReferencedPayloadKind::Relations => Self::Relations,
        }
    }
}

impl From<molgfx::core::ReferencedPayloadKind> for PyReferencedPayloadKind {
    fn from(value: molgfx::core::ReferencedPayloadKind) -> Self {
        match value {
            molgfx::core::ReferencedPayloadKind::Structure => Self::Structure,
            molgfx::core::ReferencedPayloadKind::Property => Self::Property,
            molgfx::core::ReferencedPayloadKind::Frames => Self::Frames,
            molgfx::core::ReferencedPayloadKind::Brick => Self::Brick,
            molgfx::core::ReferencedPayloadKind::Mesh => Self::Mesh,
            molgfx::core::ReferencedPayloadKind::Proxy => Self::Proxy,
            molgfx::core::ReferencedPayloadKind::Points => Self::Points,
            molgfx::core::ReferencedPayloadKind::Instances => Self::Instances,
            molgfx::core::ReferencedPayloadKind::Attribute => Self::Attribute,
            molgfx::core::ReferencedPayloadKind::Relations => Self::Relations,
        }
    }
}

/// Location-independent reference to one external payload.
#[pyclass(name = "PayloadReference", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPayloadReference(pub(crate) molgfx::core::PayloadReference);

#[pymethods]
impl PyPayloadReference {
    /// Describes an encoded payload by hashing it, so the length and the
    /// address always agree with the bytes they name.
    #[staticmethod]
    fn of(
        kind: PyReferencedPayloadKind,
        dataset: u64,
        chunk: u64,
        payload: &[u8],
    ) -> PyResult<Self> {
        let byte_len =
            u64::try_from(payload.len()).map_err(|error| crate::error::value(error.to_string()))?;
        Ok(Self(molgfx::core::PayloadReference {
            kind: kind.into(),
            dataset,
            chunk,
            byte_len,
            address: molgfx::core::ContentAddress::digest(payload),
        }))
    }

    /// Semantic payload class.
    #[getter]
    fn kind(&self) -> PyReferencedPayloadKind {
        self.0.kind.into()
    }

    /// Caller-owned global dataset identity.
    #[getter]
    fn dataset(&self) -> u64 {
        self.0.dataset
    }

    /// Chunk identity within the dataset.
    #[getter]
    fn chunk(&self) -> u64 {
        self.0.chunk
    }

    /// Exact encoded byte length.
    #[getter]
    fn byte_len(&self) -> u64 {
        self.0.byte_len
    }

    /// SHA-256 of the encoded payload bytes.
    #[getter]
    fn address(&self) -> PyContentAddress {
        PyContentAddress(self.0.address)
    }

    fn __repr__(&self) -> String {
        format!(
            "PayloadReference(kind={:?}, dataset={}, chunk={}, byte_len={})",
            self.kind(),
            self.0.dataset,
            self.0.chunk,
            self.0.byte_len
        )
    }
}

/// Scene composition plus immutable external payload references.
#[pyclass(name = "SceneManifest", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PySceneManifest(pub(crate) molgfx::core::SceneManifest);

#[pymethods]
impl PySceneManifest {
    /// Scene-owned declarative composition.
    #[getter]
    fn scene(&self) -> PySceneDescription {
        PySceneDescription(self.0.scene.clone())
    }

    /// Canonically ordered external payload references.
    #[getter]
    fn payloads(&self) -> Vec<PyPayloadReference> {
        self.0
            .payloads
            .iter()
            .copied()
            .map(PyPayloadReference)
            .collect()
    }

    /// Merkle root over every payload reference.
    #[getter]
    fn merkle_root(&self) -> PyContentAddress {
        PyContentAddress(self.0.merkle_root)
    }

    /// The manifest encoded as JSON, the exact bytes `write_manifest` streams.
    fn copy_json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let mut encoded = Vec::new();
        manifest(molgfx::core::write_manifest(&mut encoded, &self.0))?;
        Ok(PyBytes::new(py, &encoded))
    }

    fn __repr__(&self) -> String {
        format!(
            "SceneManifest(payloads={}, merkle_root={})",
            self.0.payloads.len(),
            self.merkle_root().__str__()
        )
    }
}

/// Reads and validates a manifest from a file.
#[pyfunction]
pub(crate) fn read_manifest(path: PathBuf) -> PyResult<PySceneManifest> {
    let file = std::fs::File::open(path).map_err(io_error)?;
    manifest(molgfx::core::read_manifest(file)).map(PySceneManifest)
}

/// Writes a validated manifest to a file.
#[pyfunction]
pub(crate) fn write_manifest(manifest: PySceneManifest, path: PathBuf) -> PyResult<()> {
    let file = std::fs::File::create(path).map_err(io_error)?;
    crate::error::manifest(molgfx::core::write_manifest(file, &manifest.0))
}

/// Turns a filesystem failure into the manifest exception, so a caller catches
/// one type whether the failure came from the disk or from the schema.
fn io_error(error: std::io::Error) -> PyErr {
    crate::error::manifest_error(molgfx::core::ManifestError::Io(error))
}
