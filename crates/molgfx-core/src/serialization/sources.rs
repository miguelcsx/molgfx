//! Caller-owned payloads that rehydrate a description.

/// Caller-owned payloads required to rehydrate a scene description.
///
/// Structures and dense grids remain outside the JSON manifest. Slices are
/// matched in manifest order and every payload is fingerprinted before any
/// scene table is mutated.
#[derive(Clone, Copy, Debug)]
pub struct SceneDescriptionSources<'a> {
    /// Source structures in manifest order.
    pub structures: &'a [molframe::Structure],
    /// Scalar volumes in manifest order.
    pub volumes: &'a [crate::ScalarVolume],
    /// Categorical volumes in manifest order.
    pub segmentations: &'a [crate::SegmentedVolume],
    /// Atom properties in manifest order.
    pub atom_properties: &'a [crate::AtomProperty],
    /// Caller mesh sources in manifest order.
    pub meshes: &'a [crate::Mesh],
}

/// Generic immutable payloads required by schema-8 row tables.
///
/// Each slice is matched in manifest order. Values may share their original
/// `Arc` storage; reconstruction validates their content address before
/// inserting them at the exact generational identity recorded by the scene.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenericSceneDescriptionSources<'a> {
    /// Generic point batches in manifest order.
    pub point_batches: &'a [crate::PointBatch],
    /// Shared-template instance batches in manifest order.
    pub instance_batches: &'a [crate::InstanceBatch],
    /// Typed immutable columns in manifest order.
    pub attributes: &'a [crate::AttributeColumn],
    /// Generic relation batches in manifest order.
    pub relation_batches: &'a [crate::RelationBatch],
}
