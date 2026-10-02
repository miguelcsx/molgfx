//! What a streaming request asks for and what comes back.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Priority class used by the renderer's prefetch scheduler.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Required for the next visible frame.
    Visible,
    /// Predicted to become visible soon.
    Prefetch,
    /// Opportunistic background request.
    Background,
}

/// Provider-neutral dataset metadata.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Metadata {
    /// Stable dataset identity.
    pub identity: Box<str>,
    /// Total logical chunks when known.
    pub chunk_count: Option<u64>,
    /// Approximate uncompressed bytes when known.
    pub byte_length: Option<u64>,
}

/// One prioritized batch of logical chunks.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Request {
    /// Logical chunk identities in provider order.
    pub chunks: Vec<u64>,
    /// Scheduling priority.
    pub priority: Priority,
}

/// One immutable binary chunk returned by a provider.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Chunk {
    /// Logical chunk identity.
    pub id: u64,
    /// Provider-owned bytes shared without an extra copy.
    pub bytes: Arc<[u8]>,
}
