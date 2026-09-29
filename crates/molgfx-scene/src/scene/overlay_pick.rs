//! Resolving a renderer pick to the overlay item the scene owns.
//!
//! Labels, measurements and volume segments are the only picks whose semantic
//! owner is a scene item rather than a molecular row. They share the
//! renderer's pick record but carry a storage row and an entity kind that
//! together identify which item was hit, so resolution stays `O(items)` in the
//! worst case and `O(1)` for the common single-item scene.

use super::Scene;
use crate::id::StructureId;
use crate::overlay::{measurement_anchors, measurement_shape};
use crate::{ResolvedLabelPick, ResolvedMeasurementPick, ResolvedPick, ResolvedVolumeSegmentPick};

impl Scene {
    /// Resolves a pick with no provider dataset into the overlay item the
    /// renderer's own record names.
    ///
    /// A kind this scene cannot match falls back to the renderer's own
    /// [`crate::PickResult`], so a pick of a guided or instanced entity still
    /// resolves.
    pub(super) fn overlay_pick(&self, pick: &crate::PickResult) -> ResolvedPick {
        let resolved = match pick.kind {
            crate::PickKind::Label => self.label_pick(pick),
            crate::PickKind::Measurement => self.measurement_pick(pick),
            _ => None,
        };
        if let Some(resolved) = resolved {
            return resolved;
        }
        ResolvedPick::NonAtom(pick.clone())
    }

    /// Resolves a label pick to the annotation whose storage row it names.
    fn label_pick(&self, pick: &crate::PickResult) -> Option<ResolvedPick> {
        let row = u32::try_from(pick.row?).ok()?;
        let (id, _) = self
            .overlay
            .labels
            .iter()
            .find(|(_, handle)| handle.row() == row)?;
        let spec = self.spec.annotations.get(id)?;
        Some(ResolvedPick::Label(Box::new(ResolvedLabelPick {
            structure: self.anchor_structure(&spec.anchor),
            annotation: *id,
            kind: "note".to_owned(),
            text: spec.text.to_string(),
            label: format!("label '{}'", spec.text),
        })))
    }

    /// Resolves a measurement pick to the measurement whose storage row it
    /// names.
    fn measurement_pick(&self, pick: &crate::PickResult) -> Option<ResolvedPick> {
        let row = u32::try_from(pick.row?).ok()?;
        let (id, _) = self
            .overlay
            .measurements
            .iter()
            .find(|(_, handle)| handle.row() == row)?;
        let spec = self.spec.measurements.get(id)?;
        let (kind, arity) = measurement_shape(spec);
        Some(ResolvedPick::Measurement(Box::new(
            ResolvedMeasurementPick {
                structure: self.measurement_structure(spec),
                measurement: *id,
                kind: kind.to_owned(),
                arity,
                value: None,
                label: format!("{kind} measurement"),
            },
        )))
    }

    /// Resolves the structure a measurement reads its anchors in.
    ///
    /// An anchor that names a structure wins; otherwise the scene's only
    /// structure is the reading, and a multi-structure scene reports none
    /// rather than guessing.
    fn measurement_structure(&self, spec: &crate::overlay::MeasurementSpec) -> StructureId {
        // A measurement has at least one anchor by construction, so the first
        // anchor decides; an empty spec would fall back to the stand-in.
        match measurement_anchors(spec).first() {
            Some(anchor) => self.anchor_structure(anchor),
            None => self.stand_in_structure(),
        }
    }

    /// The structure an anchor reads in.
    ///
    /// A world anchor belongs to no structure, so the scene's only one stands
    /// in; a multi-structure scene has no single answer and reports the
    /// sentinel rather than guessing.
    fn anchor_structure(&self, anchor: &crate::Anchor) -> StructureId {
        match anchor.structure() {
            Some(structure) => structure,
            None => self.stand_in_structure(),
        }
    }

    fn stand_in_structure(&self) -> StructureId {
        match self.structures.keys().next() {
            Some(structure) => *structure,
            None => StructureId(0),
        }
    }

    /// Resolves a categorical volume segment to its volume and caller label.
    ///
    /// The renderer's segment pick names the volume through its own handle; a
    /// scene that owns exactly one volume resolves it, and one that owns none
    /// leaves the pick as the renderer's record.
    pub(super) fn volume_segment_pick(
        &self,
        pick: &crate::PickResult,
    ) -> Option<ResolvedVolumeSegmentPick> {
        let label = pick.volume_label?;
        let (volume, _) = self.spec.volumes.first_key_value()?;
        Some(ResolvedVolumeSegmentPick {
            volume: *volume,
            volume_label: label,
            label: format!("volume segment {label}"),
        })
    }
}
