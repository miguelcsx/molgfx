//! The scene: placed structures, stored selections and representations.
//!
//! The scene is the renderable model. It owns handles, per-table revisions
//! and the representation list; it holds no GPU state and issues no device
//! commands. Structure edits never touch coordinates — those are borrowed —
//! so a coordinate change is a swap of the referenced model, tracked by the
//! parser's own generation counter.

use crate::SegmentedVolume;
use crate::atoms::AtomTable;
use crate::density::DensityVolume;
use crate::error::CoreError;
use crate::handle::{
    RepresentationHandle, SelectionHandle, SlotMap, StructureHandle, VolumeHandle,
};
use crate::placed::PlacedStructure;
use crate::representation::{Representation, RepresentationKind, RepresentationTarget};
#[path = "representation_state.rs"]
mod representation_state;
#[path = "world_bound.rs"]
mod world_bound;

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;

/// The renderable model of one or more structures.
#[derive(Debug, Default)]
pub struct Scene {
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
    pub(crate) ensembles: SlotMap<crate::Ensemble>,
    pub(crate) primitive: SlotMap<crate::Primitive>,
    /// Bumped whenever structure placement or membership changes.
    structure_revision: u64,
    /// Bumped whenever the representation list or its parameters change;
    /// keys the renderer's slot table rebuild.
    pub(crate) representation_revision: u64,
    pub(crate) mesh_revision: u64,
    pub(crate) overlay_revision: u64,
    /// Bumped whenever volume membership or content changes.
    volume_revision: u64,
    /// Bumped whenever categorical-volume membership or content changes.
    pub(crate) segmentation_revision: u64,
    /// Bumped whenever the interaction table or its presentation changes.
    pub(crate) interaction_revision: u64,
    pub(crate) guide_revision: u64,
    /// Bumped whenever annotations or measurements change.
    pub(crate) label_revision: u64,
    /// Bumped whenever a caller property is added, replaced or removed.
    pub(crate) property_revision: u64,
    /// Bumped whenever weighted ensemble membership changes.
    pub(crate) ensemble_revision: u64,
    /// Bumped whenever caller-authored analytic primitives change.
    pub(crate) primitive_revision: u64,
    pub(crate) spatial_traversal: Vec<u32>,
    pub(crate) spatial_candidates: Vec<u32>,
    pub(crate) spatial_result: roaring::RoaringBitmap,
    pub(crate) selection_cache: Vec<(Box<str>, crate::Select)>,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredRepresentation {
    pub(crate) value: Representation,
    pub(crate) revision: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredVolume {
    pub(crate) value: DensityVolume,
    pub(crate) revision: u64,
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
    pub fn from_structure(structure: &pdbiox::Structure) -> Result<Self, CoreError> {
        let mut scene = Self::new();
        scene.add_structure(structure)?;
        Ok(scene)
    }

    /// Places a structure into the scene at the identity transform.
    ///
    /// # Errors
    ///
    /// Fails when the structure carries no dense coordinate block to borrow.
    pub fn add_structure(
        &mut self,
        structure: &pdbiox::Structure,
    ) -> Result<StructureHandle, CoreError> {
        let placed = PlacedStructure::new(structure).ok_or(CoreError::StructureRead {
            summary: "structure has no dense coordinate block".to_owned(),
        })?;
        self.structure_revision = self.structure_revision.wrapping_add(1);
        Ok(StructureHandle(self.structures.insert(placed)))
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
    /// `pdbiox`-supplied assignments. Unmentioned residues become coil;
    /// out-of-range records are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed structure.
    pub fn apply_secondary_structure(
        &mut self,
        handle: StructureHandle,
        records: &[(pdbiox::ResidueIndex, crate::SecondaryStructure)],
    ) -> Result<(), CoreError> {
        let placed = self.structure_mut(handle).ok_or(CoreError::StaleHandle)?;
        let values = placed.secondary_structure.values_mut();
        values.fill(crate::SecondaryStructure::Coil);
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
        self.structures.iter().next().map(|(_, s)| &s.atoms)
    }

    /// Stores an immutable shared density grid.
    pub fn add_volume(&mut self, volume: DensityVolume) -> VolumeHandle {
        self.volume_revision = self.volume_revision.wrapping_add(1);
        VolumeHandle(self.volumes.insert(StoredVolume {
            value: volume,
            revision: 0,
        }))
    }

    /// Resolves a density-grid handle.
    #[must_use]
    pub fn volume(&self, handle: VolumeHandle) -> Option<&DensityVolume> {
        self.volumes.get(handle.0).map(|stored| &stored.value)
    }

    /// Replaces a grid while preserving its stable handle.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after the grid was removed.
    pub fn replace_volume(
        &mut self,
        handle: VolumeHandle,
        volume: DensityVolume,
    ) -> Result<(), CoreError> {
        let stored = self
            .volumes
            .get_mut(handle.0)
            .ok_or(CoreError::StaleHandle)?;
        stored.value = volume;
        stored.revision = stored.revision.wrapping_add(1);
        self.volume_revision = self.volume_revision.wrapping_add(1);
        Ok(())
    }

    /// Removes a density grid and invalidates its handle.
    pub fn remove_volume(&mut self, handle: VolumeHandle) -> Option<DensityVolume> {
        let removed = self.volumes.remove(handle.0).map(|stored| stored.value);
        if removed.is_some() {
            self.volume_revision = self.volume_revision.wrapping_add(1);
        }
        removed
    }

    /// Adds a representation of `kind` over a stored selection.
    ///
    /// # Errors
    ///
    /// Fails on a stale selection handle, or a kind the engine cannot draw
    /// yet.
    pub fn represent(
        &mut self,
        selection: SelectionHandle,
        kind: RepresentationKind,
    ) -> Result<RepresentationHandle, CoreError> {
        if self.selections.get(selection.0).is_none() {
            return Err(CoreError::StaleHandle);
        }
        match kind {
            RepresentationKind::Spacefill
            | RepresentationKind::BallAndStick
            | RepresentationKind::Licorice
            | RepresentationKind::Lines
            | RepresentationKind::Cartoon
            | RepresentationKind::Trace
            | RepresentationKind::Tube
            | RepresentationKind::Surface
            | RepresentationKind::Beads
            | RepresentationKind::Rocket
            | RepresentationKind::Twister
            | RepresentationKind::PaperChain
            | RepresentationKind::Points => {}
            other => return Err(CoreError::Unsupported { kind: other }),
        }
        self.representation_revision = self.representation_revision.wrapping_add(1);
        let handle = self.representations.insert(StoredRepresentation {
            value: Representation::new(RepresentationTarget::Selection(selection), kind),
            revision: 0,
        });
        Ok(RepresentationHandle(handle))
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
        negative_color: pdviewx_math::Rgba8,
        positive_color: pdviewx_math::Rgba8,
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
        let negative = self.represent_isosurface(volume)?;
        let positive = self.represent_isosurface(volume)?;
        if let Some(representation) = self.representation_mut(negative) {
            representation.params.isolevel = negative_level;
        }
        if let Some(representation) = self.representation_mut(positive) {
            representation.params.isolevel = positive_level;
        }
        for (handle, color) in [(negative, negative_color), (positive, positive_color)] {
            if let Some(representation) = self.representation_mut(handle) {
                representation.volume.transfer = crate::VolumeTransferFunction::linear(
                    [negative_level, positive_level],
                    color,
                    color,
                );
            }
        }
        Ok(vec![negative, positive])
    }

    /// Adds direct volume rendering over one stored density grid.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after the grid was removed.
    pub fn represent_volume(
        &mut self,
        volume: VolumeHandle,
    ) -> Result<RepresentationHandle, CoreError> {
        self.represent_volume_with(volume, crate::VolumeRendering::Direct)
    }

    /// Adds a lit implicit isosurface over one stored density grid.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after the grid was removed.
    pub fn represent_isosurface(
        &mut self,
        volume: VolumeHandle,
    ) -> Result<RepresentationHandle, CoreError> {
        self.represent_volume_with(volume, crate::VolumeRendering::Isosurface)
    }

    /// Adds a participating optical medium over one caller-supplied density.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after the grid was removed.
    pub fn represent_medium(
        &mut self,
        volume: VolumeHandle,
    ) -> Result<RepresentationHandle, CoreError> {
        self.represent_volume_with(volume, crate::VolumeRendering::Medium)
    }

    /// Adds one arbitrary transfer-mapped plane through a stored scalar grid.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after the grid was removed.
    pub fn represent_volume_slice(
        &mut self,
        volume: VolumeHandle,
        slice: crate::VolumeSlice,
    ) -> Result<RepresentationHandle, CoreError> {
        let handle = self.represent_volume_with(volume, crate::VolumeRendering::Slice)?;
        if let Some(representation) = self.representations.get_mut(handle.0) {
            representation.value.volume.slice = Some(slice);
            representation.revision = representation.revision.wrapping_add(1);
        }
        Ok(handle)
    }

    /// Adds a screen-space liquid-like boundary over caller-provided density.
    ///
    /// The field remains caller-owned; this mode only changes the boundary
    /// shading and screen-space termination, never the underlying scalar data.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after the grid was removed.
    pub fn represent_liquid_surface(
        &mut self,
        volume: VolumeHandle,
    ) -> Result<RepresentationHandle, CoreError> {
        self.represent_volume_with(volume, crate::VolumeRendering::LiquidSurface)
    }

    /// Adds direct rendering over a validated region of one resident grid.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after the grid was removed.
    pub fn represent_volume_region(
        &mut self,
        volume: VolumeHandle,
        region: crate::VolumeRegion,
    ) -> Result<RepresentationHandle, CoreError> {
        let handle = self.represent_volume_with(volume, crate::VolumeRendering::Direct)?;
        if let Some(representation) = self.representations.get_mut(handle.0) {
            representation.value.volume.region = Some(region);
            representation.revision = representation.revision.wrapping_add(1);
        }
        Ok(handle)
    }

    fn represent_volume_with(
        &mut self,
        volume: VolumeHandle,
        rendering: crate::VolumeRendering,
    ) -> Result<RepresentationHandle, CoreError> {
        let range = self
            .volumes
            .get(volume.0)
            .map(|stored| stored.value.range())
            .ok_or(CoreError::StaleHandle)?;
        let mut representation = Representation::new(
            RepresentationTarget::Volume(volume),
            RepresentationKind::Volume,
        );
        representation.volume.transfer = crate::VolumeTransferFunction::linear(
            range,
            pdviewx_math::Rgba8::opaque(68, 1, 84),
            pdviewx_math::Rgba8::opaque(253, 231, 37),
        );
        representation.volume.rendering = rendering;
        representation.params.isolevel = (range[0] + range[1]) * 0.5;
        self.representation_revision = self.representation_revision.wrapping_add(1);
        Ok(RepresentationHandle(self.representations.insert(
            StoredRepresentation {
                value: representation,
                revision: 0,
            },
        )))
    }
}
