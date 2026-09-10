//! Streaming, content-addressed scene manifests.

use super::{SceneDescription, manifest::SCHEMA_VERSION};
use crate::Scene;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use thiserror::Error;

const ADDRESS_BYTES: usize = 32;

/// SHA-256 identity of an immutable external payload.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ContentAddress([u8; ADDRESS_BYTES]);

impl ContentAddress {
    /// Hashes one contiguous payload without retaining a second copy.
    #[must_use]
    pub fn digest(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    /// Returns the binary digest used by stores and Merkle nodes.
    #[must_use]
    pub const fn bytes(self) -> [u8; ADDRESS_BYTES] {
        self.0
    }
}

pub(crate) struct ContentHasher(Sha256);

impl ContentHasher {
    pub(crate) fn new() -> Self {
        Self(Sha256::new())
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    pub(crate) fn finish(self) -> ContentAddress {
        ContentAddress(self.0.finalize().into())
    }
}

/// Semantic class of one caller-owned immutable payload.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReferencedPayloadKind {
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

/// Location-independent reference to one external payload.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PayloadReference {
    /// Semantic payload class.
    pub kind: ReferencedPayloadKind,
    /// Caller-owned global dataset identity.
    pub dataset: u64,
    /// Chunk identity within the dataset.
    pub chunk: u64,
    /// Exact encoded byte length.
    pub byte_len: u64,
    /// SHA-256 of the encoded payload bytes.
    pub address: ContentAddress,
}

/// Scene composition plus immutable external payload references.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct SceneManifest {
    /// Scene-owned declarative composition.
    pub scene: SceneDescription,
    /// Canonically ordered external payload references.
    pub payloads: Vec<PayloadReference>,
    /// Merkle root over every payload reference.
    pub merkle_root: ContentAddress,
}

impl SceneManifest {
    /// Builds a canonical manifest without loading any referenced payload.
    #[must_use]
    pub fn new(scene: SceneDescription, mut payloads: Vec<PayloadReference>) -> Self {
        payloads.sort_unstable();
        let merkle_root = merkle_root(&payloads);
        Self {
            scene,
            payloads,
            merkle_root,
        }
    }

    fn validate(&self) -> Result<(), ManifestError> {
        if self.scene.schema != SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchema {
                found: self.scene.schema,
                expected: SCHEMA_VERSION,
            });
        }
        if self.scene.engine != format!("pdviewx-scene-{SCHEMA_VERSION}") {
            return Err(ManifestError::EngineMismatch);
        }
        if !self.payloads.windows(2).all(|pair| pair[0] <= pair[1]) {
            return Err(ManifestError::NonCanonicalPayloadOrder);
        }
        let actual = merkle_root(&self.payloads);
        if actual != self.merkle_root {
            return Err(ManifestError::MerkleMismatch);
        }
        Ok(())
    }
}

impl Scene {
    /// Captures scene composition and caller-supplied payload metadata without
    /// opening, hashing or retaining any physical payload.
    #[must_use]
    pub fn manifest(&self, mut payloads: Vec<PayloadReference>) -> SceneManifest {
        let scene = self.describe();
        payloads.extend(scene.point_batches.iter().map(|value| value.payload));
        payloads.extend(scene.instance_batches.iter().map(|value| value.payload));
        payloads.extend(scene.attributes.iter().map(|value| value.payload));
        payloads.extend(scene.relation_batches.iter().map(|value| value.payload));
        payloads.sort_unstable();
        payloads.dedup();
        SceneManifest::new(scene, payloads)
    }
}

/// Failure while streaming or resolving a manifest.
#[derive(Debug, Error)]
pub enum ManifestError {
    /// Underlying input or output failed.
    #[error("manifest I/O failed: {0}")]
    Io(#[from] io::Error),
    /// JSON syntax or data did not match the manifest schema.
    #[error("manifest JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    /// The manifest was produced for another schema.
    #[error("unsupported manifest schema {found}; expected {expected}")]
    UnsupportedSchema {
        /// Schema read from the stream.
        found: u16,
        /// Only accepted schema.
        expected: u16,
    },
    /// Engine format identifier did not match the current schema.
    #[error("manifest engine format does not match the current schema")]
    EngineMismatch,
    /// Payload references were not in canonical order.
    #[error("manifest payload references are not canonically ordered")]
    NonCanonicalPayloadOrder,
    /// Payload-reference Merkle root did not match.
    #[error("manifest payload Merkle root does not match")]
    MerkleMismatch,
    /// The resolver does not currently provide the requested payload.
    #[error("payload is not available from the resolver")]
    MissingPayload,
}

/// Reads a manifest directly from a stream without an intermediate string.
///
/// # Errors
///
/// Returns [`ManifestError`] when reading, decoding, schema validation or
/// Merkle validation fails.
pub fn read_manifest<R: Read>(reader: R) -> Result<SceneManifest, ManifestError> {
    let manifest: SceneManifest = serde_json::from_reader(reader)?;
    manifest.validate()?;
    Ok(manifest)
}

/// Writes a validated manifest directly to a stream.
///
/// # Errors
///
/// Returns [`ManifestError`] when validation, encoding or output fails.
pub fn write_manifest<W: Write>(writer: W, manifest: &SceneManifest) -> Result<(), ManifestError> {
    manifest.validate()?;
    serde_json::to_writer(writer, manifest)?;
    Ok(())
}

/// Caller-provided lazy access to physical payload storage.
pub trait PayloadResolver {
    /// Reader returned by the backing store.
    type Reader: Read;

    /// Opens a payload, or returns `None` while it is unavailable.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when the backing store cannot resolve the
    /// reference.
    fn open(&self, reference: &PayloadReference) -> Result<Option<Self::Reader>, ManifestError>;
}

/// Manifest metadata paired with a caller-owned lazy resolver.
#[derive(Debug)]
pub struct LazyManifest<R> {
    manifest: SceneManifest,
    resolver: R,
}

impl<R: PayloadResolver> LazyManifest<R> {
    /// Couples validated metadata to its storage resolver without performing I/O.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when schema, ordering or Merkle validation
    /// fails.
    pub fn new(manifest: SceneManifest, resolver: R) -> Result<Self, ManifestError> {
        manifest.validate()?;
        Ok(Self { manifest, resolver })
    }

    /// Returns the immutable manifest metadata.
    #[must_use]
    pub const fn manifest(&self) -> &SceneManifest {
        &self.manifest
    }

    /// Opens one payload and verifies its length and digest as it is consumed.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when the resolver fails or does not currently
    /// provide the payload.
    pub fn open(
        &self,
        reference: &PayloadReference,
    ) -> Result<VerifiedPayload<R::Reader>, ManifestError> {
        let Some(reader) = self.resolver.open(reference)? else {
            return Err(ManifestError::MissingPayload);
        };
        Ok(VerifiedPayload::new(reader, *reference))
    }
}

/// Reader that checks content address and length when EOF is reached.
#[derive(Debug)]
pub struct VerifiedPayload<R> {
    inner: R,
    expected: PayloadReference,
    hasher: Sha256,
    bytes_read: u64,
    verified: bool,
}

impl<R> VerifiedPayload<R> {
    fn new(inner: R, expected: PayloadReference) -> Self {
        Self {
            inner,
            expected,
            hasher: Sha256::new(),
            bytes_read: 0,
            verified: false,
        }
    }
}

impl<R: Read> Read for VerifiedPayload<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        if count == 0 {
            if !self.verified {
                let actual: [u8; ADDRESS_BYTES] = self.hasher.clone().finalize().into();
                if self.bytes_read != self.expected.byte_len || actual != self.expected.address.0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "resolved payload does not match its content address",
                    ));
                }
                self.verified = true;
            }
            return Ok(0);
        }
        let increment = u64::try_from(count)
            .map_err(|_| io::Error::other("payload read length exceeds u64"))?;
        self.bytes_read = self
            .bytes_read
            .checked_add(increment)
            .ok_or_else(|| io::Error::other("payload read length overflow"))?;
        self.hasher.update(&buffer[..count]);
        Ok(count)
    }
}

fn merkle_root(payloads: &[PayloadReference]) -> ContentAddress {
    let mut level: Vec<ContentAddress> = payloads.iter().map(reference_hash).collect();
    if level.is_empty() {
        return hash_parts(&[b"pdviewx-empty-merkle-v1"]);
    }
    while level.len() > 1 {
        let mut parents = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let right = if pair.len() == 2 { pair[1] } else { pair[0] };
            parents.push(hash_parts(&[
                b"pdviewx-merkle-node-v1",
                &pair[0].0,
                &right.0,
            ]));
        }
        level = parents;
    }
    level[0]
}

fn reference_hash(reference: &PayloadReference) -> ContentAddress {
    let kind = [reference.kind as u8];
    hash_parts(&[
        b"pdviewx-payload-reference-v1",
        &kind,
        &reference.dataset.to_le_bytes(),
        &reference.chunk.to_le_bytes(),
        &reference.byte_len.to_le_bytes(),
        &reference.address.0,
    ])
}

fn hash_parts(parts: &[&[u8]]) -> ContentAddress {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    ContentAddress(hasher.finalize().into())
}

#[cfg(test)]
#[path = "manifest_io_tests.rs"]
mod tests;
