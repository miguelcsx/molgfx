//! Stable semantic scene identifiers.

use serde::{Deserialize, Serialize};

macro_rules! semantic_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize)]
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
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct IdentityVisitor;

                impl serde::de::Visitor<'_> for IdentityVisitor {
                    type Value = $name;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("a nonnegative semantic identity")
                    }

                    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                        Ok($name(value))
                    }

                    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                        value.parse::<u64>().map($name).map_err(E::custom)
                    }
                }

                deserializer.deserialize_any(IdentityVisitor)
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
    InteractionId,
    "Identity of an overlay interaction in one scene."
);
semantic_id!(TrajectoryId, "Identity of a trajectory in one scene.");
semantic_id!(
    EllipsoidId,
    "Identity of a per-atom anisotropic-displacement ellipsoid overlay in one scene."
);
semantic_id!(
    PlaneId,
    "Identity of a caller-authored planar guide in one scene."
);
semantic_id!(
    AppearanceRuleId,
    "Identity of a selection-scoped appearance rule in one scene. A higher identity takes precedence where rules overlap."
);

/// Identity of one categorical-grid lifetime; a reused slot has a new generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SegmentationId {
    /// Domain slot.
    pub index: u64,
    /// Slot lifetime generation.
    pub generation: u64,
}
impl SegmentationId {
    /// Reconstructs both identity components from an authoring protocol.
    #[must_use]
    pub const fn new(index: u64, generation: u64) -> Self {
        Self { index, generation }
    }
}
impl Serialize for SegmentationId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("{}:{}", self.index, self.generation))
    }
}
impl<'de> Deserialize<'de> for SegmentationId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let (index, generation) = value.split_once(':').ok_or_else(|| {
            serde::de::Error::custom("segmentation identity requires index:generation")
        })?;
        Ok(Self {
            index: index.parse().map_err(serde::de::Error::custom)?,
            generation: generation.parse().map_err(serde::de::Error::custom)?,
        })
    }
}
