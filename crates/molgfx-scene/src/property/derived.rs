//! Bindings whose identity is derived from their own values.

use super::ScalarPropertyBinding;
use crate::{DataSource, Error, StructureId};
use sha2::{Digest as _, Sha256};
use std::sync::Arc;

/// Hex characters of the content hash kept in a derived property name.
const NAME_HASH_CHARS: usize = 12;

impl ScalarPropertyBinding {
    /// Declares a column whose source hash is the SHA-256 of its values.
    ///
    /// Equal columns hash equally, so the binding is reproducible without the
    /// caller inventing an identity. `name` defaults to `values-` followed by
    /// the first twelve hex characters of the hash. An empty column is
    /// rejected because there is nothing to derive a display domain from.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidSpec`] when `values` is empty.
    pub fn derived(
        structure: StructureId,
        name: Option<&str>,
        values: Arc<[f32]>,
    ) -> Result<Self, Error> {
        if values.is_empty() {
            return Err(Error::InvalidSpec("values must not be empty".to_owned()));
        }
        let mut hash = Sha256::new();
        for value in values.iter() {
            hash.update(value.to_le_bytes());
        }
        let digest = format!("{:x}", hash.finalize());
        let name = match name {
            Some(name) => name.to_owned(),
            None => format!("values-{}", &digest[..NAME_HASH_CHARS]),
        };
        Ok(Self::new(structure, name, DataSource::new(digest), values))
    }

    /// The finite display domain this binding currently carries.
    #[must_use]
    pub fn display_domain(&self) -> [f32; 2] {
        self.spec.domain
    }
}
