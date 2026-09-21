//! The scene: placed structures, stored selections and representations.
//!
//! The scene is the renderable model. It owns handles, per-table revisions
//! and the representation list; it holds no GPU state and issues no device
//! commands. Structure edits never touch coordinates — those are borrowed —
//! so a coordinate change is a swap of the referenced model, tracked by the
//! parser's own generation counter.

use crate::SegmentedVolume;
use crate::atoms::AtomTable;
use crate::density::ScalarVolume;
use crate::error::CoreError;
use crate::handle::{RepresentationHandle, SlotMap, StructureHandle, VolumeHandle};
use crate::placed::PlacedStructure;
use crate::representation::{Representation, RepresentationKind, RepresentationTarget};
use crate::{DatasetId, StructureAsset};
#[path = "identity.rs"]
mod identity;
#[path = "representation_state.rs"]
mod representation_state;
#[path = "world_bound.rs"]
mod world_bound;
use identity::SceneIdentity;

#[cfg(test)]
#[path = "asset_tests.rs"]
mod asset_tests;
#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;

/// The renderable model of one or more structures.
#[derive(Debug, Default)]
pub struct Scene {
    identity: SceneIdentity,
    pub(crate) structures: SlotMap<PlacedStructure>,
    pub(crate) selections: SlotMap<StoredSelection>,
    pub(crate) representations: SlotMap<StoredRepresentation>,
    pub(crate) volumes: SlotMap<StoredVolume>,
    pub(crate) segmentations: SlotMap<StoredSegmentation>,
    pub(crate) interactions: SlotMap<crate::InteractionEdge>,
    pub(crate) meshes: SlotMap<crate::Mesh>,
    pub(crate) mesh_instances: SlotMap<crate::MeshInstance>,
    pub(crate) overlays: SlotMap<crate::ScreenOverlay>,
    pub(crate) guides: SlotMap<crate::Guide>,
    pub(crate) labels: SlotMap<crate::annotation::LabelObject>,
    pub(crate) properties: SlotMap<StoredAtomProperty>,
    pub(crate) primitive: SlotMap<crate::Primitive>,
    pub(crate) ligand_pose_batches: SlotMap<crate::LigandPoseBatch>,
    pub(crate) point_batches: SlotMap<crate::structure::PointBatch>,
    pub(crate) instance_batches: SlotMap<crate::structure::InstanceBatch>,
    pub(crate) relation_batches: SlotMap<crate::representation::RelationBatch>,
    pub(crate) attributes: SlotMap<StoredAttribute>,
    pub(crate) ensembles: SlotMap<crate::Ensemble>,
    pub(crate) domain_visuals:
        std::collections::BTreeMap<crate::RowDomain, crate::VisualDescriptor>,
    pub(crate) instance_timeline:
        std::collections::BTreeMap<crate::InstanceBatchHandle, TemporalInstances>,
    pub(crate) point_timeline: std::collections::BTreeMap<crate::PointBatchHandle, TemporalPoints>,
    pub(crate) attribute_timeline:
        std::collections::BTreeMap<crate::AttributeHandle, TemporalAttribute>,
    /// Bumped whenever structure placement or membership changes.
    structure_revision: u64,
    /// Bumped whenever the representation list or its parameters change;
    /// keys the renderer's slot table rebuild.
    pub(crate) representation_revision: u64,
    pub(crate) mesh_revision: u64,
    pub(crate) overlay_revision: u64,
    /// Bumped whenever volume membership or content changes.
    pub(crate) volume_revision: u64,
    /// Bumped whenever categorical-volume membership or content changes.
    pub(crate) segmentation_revision: u64,
    /// Bumped whenever the interaction table or its presentation changes.
    pub(crate) interaction_revision: u64,
    pub(crate) guide_revision: u64,
    /// Bumped whenever annotations or measurements change.
    pub(crate) label_revision: u64,
    /// Bumped whenever a caller property is added, replaced or removed.
    pub(crate) property_revision: u64,
    /// Bumped whenever caller-authored analytic primitives change.
    pub(crate) primitive_revision: u64,
    /// Bumped when generic point, instance or relation membership changes.
    pub(crate) generic_batch_revision: u64,
    /// Bumped when typed attribute membership or content changes.
    pub(crate) attribute_revision: u64,
    /// Bumped when a generic domain visual or its parameters change.
    pub(crate) domain_visual_revision: u64,
    /// Bumped whenever weighted ensemble membership changes.
    pub(crate) ensemble_revision: u64,
    /// Caller-controlled global presentation clock consumed by visual programs.
    pub(crate) presentation_time_seconds: f32,
    pub(crate) presentation_revision: u64,
    pub(crate) generic_timeline_binding_revision: u64,
    pub(crate) spatial_traversal: Vec<u32>,
    pub(crate) spatial_candidates: Vec<u32>,
    pub(crate) spatial_result: roaring::RoaringBitmap,
    pub(crate) selection_cache: Vec<(Box<str>, crate::Select)>,
}

#[derive(Clone, Debug)]
pub(crate) struct TemporalInstances {
    pub(crate) start: std::sync::Arc<[crate::RigidInstance]>,
    pub(crate) end: std::sync::Arc<[crate::RigidInstance]>,
    pub(crate) alpha: f32,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct TemporalPoints {
    pub(crate) start: std::sync::Arc<[[f32; 3]]>,
    pub(crate) end: std::sync::Arc<[[f32; 3]]>,
    pub(crate) alpha: f32,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct TemporalAttribute {
    pub(crate) start: crate::AttributeValues,
    pub(crate) end: crate::AttributeValues,
    pub(crate) alpha: f32,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredRepresentation {
    pub(crate) value: Representation,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredVolume {
    pub(crate) value: Option<ScalarVolume>,
    pub(crate) occupancy: Option<BoundOccupancy>,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct BoundOccupancy {
    pub(crate) stream: crate::OccupancyStream,
    pub(crate) structure: StructureHandle,
    pub(crate) atom_rows: std::sync::Arc<[u32]>,
}

impl StoredVolume {
    pub(crate) fn range(&self) -> [f32; 2] {
        self.value.as_ref().map_or_else(
            || {
                self.occupancy
                    .as_ref()
                    .map_or([0.0, 1.0], |value| [0.0, value.stream.maximum()])
            },
            ScalarVolume::range,
        )
    }

    pub(crate) fn world_aabb(&self, scene: &Scene) -> molgfx_math::Aabb {
        if let Some(value) = &self.value {
            return value.world_aabb();
        }
        let Some(occupancy) = &self.occupancy else {
            return molgfx_math::Aabb::EMPTY;
        };
        scene.structure(occupancy.structure).map_or_else(
            || molgfx_math::Aabb::EMPTY,
            |placed| {
                occupancy
                    .stream
                    .model_aabb()
                    .transform(&placed.model_to_world)
            },
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StoredSegmentation {
    pub(crate) value: SegmentedVolume,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredAtomProperty {
    pub(crate) value: crate::AtomProperty,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredAttribute {
    pub(crate) value: crate::representation::AttributeColumn,
    pub(crate) revision: u64,
    pub(crate) dirty_rows: std::ops::Range<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredSelection {
    pub(crate) global: Option<crate::AtomSelection>,
    pub(crate) scoped: Vec<(StructureHandle, crate::AtomSelection)>,
}

impl Scene {
    /// An empty scene.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a scene containing one structure and no representations.
    ///
    /// # Errors
    ///
    /// Fails when the structure carries no dense coordinate block to borrow.
    pub fn from_structure(structure: &molframe::Structure) -> Result<Self, CoreError> {
        let asset = StructureAsset::new(DatasetId::LEGACY, structure)
            .map_err(|error| structure_asset_error(&error))?;
        Ok(Self::from_asset(&asset))
    }

    /// Builds a scene containing one placement of a shared structure asset.
    #[must_use]
    pub fn from_asset(asset: &StructureAsset) -> Self {
        let mut scene = Self::new();
        scene.add_asset(asset);
        scene
    }

    /// Places a structure into the scene at the identity transform.
    ///
    /// # Errors
    ///
    /// Fails when the structure carries no dense coordinate block to borrow.
    pub fn add_structure(
        &mut self,
        structure: &molframe::Structure,
    ) -> Result<StructureHandle, CoreError> {
        let asset = StructureAsset::new(DatasetId::LEGACY, structure)
            .map_err(|error| structure_asset_error(&error))?;
        Ok(self.add_asset(&asset))
    }

    /// Places a shared structure asset at the identity transform.
    pub fn add_asset(&mut self, asset: &StructureAsset) -> StructureHandle {
        let placed = PlacedStructure::from_asset(asset);
        self.structure_revision = self.structure_revision.wrapping_add(1);
        StructureHandle(self.structures.insert(placed))
    }

    /// Removes a placed structure; its handle and dependent representations
    /// become stale.
    pub fn remove_structure(&mut self, handle: StructureHandle) -> Option<PlacedStructure> {
        let removed = self.structures.remove(handle.0);
        if removed.is_some() {
            self.structure_revision = self.structure_revision.wrapping_add(1);
        }
        removed
    }

    /// Resolves a structure handle.
    #[must_use]
    pub fn structure(&self, handle: StructureHandle) -> Option<&PlacedStructure> {
        self.structures.get(handle.0)
    }

    /// Mutable resolution of a structure handle.
    pub fn structure_mut(&mut self, handle: StructureHandle) -> Option<&mut PlacedStructure> {
        self.structures.get_mut(handle.0)
    }

    /// Replaces the residue secondary-structure column from caller- or
    /// `molframe`-supplied assignments. Unmentioned residues remain unknown;
    /// out-of-range records are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed structure.
    pub fn apply_secondary_structure(
        &mut self,
        handle: StructureHandle,
        records: &[(molframe::ResidueIndex, crate::SecondaryStructure)],
    ) -> Result<(), CoreError> {
        let placed = self.structure_mut(handle).ok_or(CoreError::StaleHandle)?;
        let values = placed.secondary_structure.values_mut();
        values.fill(crate::SecondaryStructure::Unknown);
        for &(residue, kind) in records {
            let index = residue.as_usize();
            if let Some(value) = values.get_mut(index) {
                *value = kind;
            }
        }
        Ok(())
    }

    /// Iterates placed structures in stable slot order.
    pub fn structures(&self) -> impl Iterator<Item = (StructureHandle, &PlacedStructure)> + '_ {
        self.structures.iter().map(|(h, s)| (StructureHandle(h), s))
    }

    /// The first placed structure's atom table, if any; the common case of a
    /// single-structure scene.
    #[must_use]
    pub fn first_atoms(&self) -> Option<&AtomTable> {
        self.structures
            .iter()
            .next()
            .map(|(_, structure)| structure.atoms.as_ref())
    }

    /// Stores an immutable shared density grid.
    pub fn add_volume(&mut self, volume: ScalarVolume) -> VolumeHandle {
        self.volume_revision = self.volume_revision.wrapping_add(1);
        VolumeHandle(self.volumes.insert(StoredVolume {
            value: Some(volume),
            occupancy: None,
            revision: 0,
        }))
    }

    /// Resolves a density-grid handle.
    #[must_use]
    pub fn volume(&self, handle: VolumeHandle) -> Option<&ScalarVolume> {
        self.volumes
            .get(handle.0)
            .and_then(|stored| stored.value.as_ref())
    }

    /// Replaces a grid while preserving its stable handle.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after the grid was removed.
    pub fn replace_volume(
        &mut self,
        handle: VolumeHandle,
        volume: ScalarVolume,
    ) -> Result<(), CoreError> {
        let stored = self
            .volumes
            .get_mut(handle.0)
            .ok_or(CoreError::StaleHandle)?;
        stored.value = Some(volume);
        stored.occupancy = None;
        stored.revision = stored.revision.wrapping_add(1);
        self.volume_revision = self.volume_revision.wrapping_add(1);
        Ok(())
    }

    /// Removes a density grid and invalidates its handle.
    pub fn remove_volume(&mut self, handle: VolumeHandle) -> Option<ScalarVolume> {
        let removed = self.volumes.remove(handle.0);
        if removed.is_some() {
            self.volume_revision = self.volume_revision.wrapping_add(1);
        }
        removed.and_then(|stored| stored.value)
    }

    /// Adds one declarative preset over a compatible selection or volume.
    ///
    /// Signed isosurfaces return two handles in negative-then-positive order;
    /// every other preset returns one.
    ///
    /// # Errors
    ///
    /// Returns a typed error for stale targets, mismatched target families or
    /// malformed signed levels.
    pub fn represent_preset(
        &mut self,
        target: RepresentationTarget,
        preset: crate::RepresentationPreset,
    ) -> Result<Vec<RepresentationHandle>, CoreError> {
        match (target, preset) {
            (RepresentationTarget::Selection(selection), crate::RepresentationPreset::Cpk) => {
                let handle = self.represent(selection, RepresentationKind::BallAndStick)?;
                if let Some(representation) = self.representation_mut(handle) {
                    representation.params.radius_scale = 0.7;
                    representation.params.bond_radius = 0.3;
                }
                Ok(vec![handle])
            }
            (RepresentationTarget::Selection(selection), crate::RepresentationPreset::Licorice) => {
                Ok(vec![
                    self.represent(selection, RepresentationKind::Licorice)?,
                ])
            }
            (
                RepresentationTarget::Selection(selection),
                crate::RepresentationPreset::PaperChain,
            ) => Ok(vec![
                self.represent(selection, RepresentationKind::PaperChain)?,
            ]),
            (
                RepresentationTarget::Selection(selection),
                crate::RepresentationPreset::DottedSolvent,
            ) => {
                let handle = self.represent(selection, RepresentationKind::Surface)?;
                if let Some(representation) = self.representation_mut(handle) {
                    representation.params.surface_kind = crate::SurfaceKind::SolventAccessible;
                    representation.params.surface_style = crate::SurfaceStyle::Dots;
                }
                Ok(vec![handle])
            }
            (
                RepresentationTarget::Volume(volume),
                crate::RepresentationPreset::SignedIsosurface {
                    negative_level,
                    positive_level,
                    negative_color,
                    positive_color,
                },
            ) => self.represent_signed_isosurfaces(
                volume,
                negative_level,
                positive_level,
                negative_color,
                positive_color,
            ),
            _ => Err(CoreError::InvalidSelection {
                reason: "representation preset is incompatible with its target",
            }),
        }
    }

    fn represent_signed_isosurfaces(
        &mut self,
        volume: VolumeHandle,
        negative_level: f32,
        positive_level: f32,
        negative_color: molgfx_math::Rgba8,
        positive_color: molgfx_math::Rgba8,
    ) -> Result<Vec<RepresentationHandle>, CoreError> {
        if !negative_level.is_finite()
            || !positive_level.is_finite()
            || negative_level >= 0.0
            || positive_level <= 0.0
        {
            return Err(CoreError::InvalidVolume {
                reason: "signed isosurfaces require one negative and one positive level",
            });
        }
        let recipe = |level, color| {
            crate::Representation::volume()
                .isolevel(level)
                .volume_style(crate::VolumeStyle::isosurface().transfer(
                    crate::VolumeTransferFunction::linear(
                        [negative_level, positive_level],
                        color,
                        color,
                    ),
                ))
        };
        let negative = self.represent(volume, recipe(negative_level, negative_color))?;
        let positive = self.represent(volume, recipe(positive_level, positive_color))?;
        Ok(vec![negative, positive])
    }
}

fn structure_asset_error(error: &crate::DatasetError) -> CoreError {
    CoreError::StructureRead {
        summary: error.to_string(),
    }
}
