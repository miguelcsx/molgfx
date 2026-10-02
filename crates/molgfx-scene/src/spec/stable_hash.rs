//! Stable content hashing of serializable values.

use serde::Serialize;

pub(crate) fn stable_json_hash(value: &impl Serialize) -> String {
    use sha2::Digest as _;
    let bytes = match serde_json::to_vec(value) {
        Ok(bytes) => bytes,
        Err(error) => error.to_string().into_bytes(),
    };
    format!("{:x}", sha2::Sha256::digest(bytes))
}
