//! Stable semantic scene identifiers.

use serde::{Deserialize, Serialize};

macro_rules! semantic_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub(crate) u64);

        impl $name {
            /// Reconstructs an identity received through a serialized protocol.
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            /// Stable integer value used by serialized patches.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

semantic_id!(StructureId, "Identity of a molecular source in one scene.");
semantic_id!(
    RepresentationId,
    "Identity of a representation in one scene."
);
