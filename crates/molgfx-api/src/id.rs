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
semantic_id!(VolumeId, "Identity of a density volume in one scene.");
semantic_id!(AnnotationId, "Identity of an annotation in one scene.");
semantic_id!(MeasurementId, "Identity of a measurement in one scene.");
semantic_id!(
    ScientificInteractionId,
    "Identity of a scientific interaction in one scene."
);
semantic_id!(TrajectoryId, "Identity of a trajectory in one scene.");
semantic_id!(
    AppearanceRuleId,
    "Identity of a selection-scoped appearance rule in one scene. A higher identity takes precedence where rules overlap."
);
