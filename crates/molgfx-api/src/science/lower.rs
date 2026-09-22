//! Lowering portable scientific descriptors onto the caller-facing core scene.
//!
//! Descriptors carry anchors, classification and provenance; bulk data lives in
//! runtime bindings. This module is the only place the two meet, and it supplies
//! exactly what the renderer consumes: caller-resolved anchors, caller-computed
//! measurement values and caller-declared interaction classes. No chemistry is
//! inferred here.

use crate::error::Error;
use crate::id::{AnnotationId, MeasurementId, ScientificInteractionId, StructureId, VolumeId};
use crate::representation::Selection;
use crate::science::{
    Anchor, MeasurementSpec, ScienceBindings, ScientificInteractionSpec, VolumeSpec,
};
use molgfx_core::{
    Annotation, AnnotationAnchor, AnnotationHandle, InteractionAnchor, InteractionEdge,
    InteractionGeometry, InteractionHandle, Measurement, MeasurementHandle, MolecularSource, Scene,
    SelectionHandle, StructureHandle, VolumeHandle,
};
use molgfx_math::Vec3;
use std::collections::BTreeMap;

/// Renderer handles a resolved scene assigned to each scientific item.
#[derive(Clone, Debug, Default)]
pub(crate) struct LoweredScience {
    pub(crate) volumes: Vec<(VolumeId, VolumeHandle)>,
    pub(crate) labels: Vec<(AnnotationId, AnnotationHandle)>,
    pub(crate) measurements: Vec<(MeasurementId, MeasurementHandle)>,
    pub(crate) interactions: Vec<(ScientificInteractionId, InteractionHandle)>,
}

impl LoweredScience {
    /// Handle counts for the public inspection view.
    pub(crate) fn counts(&self) -> crate::science::ScientificHandles {
        crate::science::ScientificHandles {
            volumes: self.volumes.len(),
            labels: self.labels.len(),
            measurements: self.measurements.len(),
            interactions: self.interactions.len(),
        }
    }
}

/// Everything lowering needs from the scene it resolves against.
pub(crate) struct SciLowering<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) structures: &'a BTreeMap<StructureId, MolecularSource>,
    pub(crate) handles: &'a BTreeMap<StructureId, StructureHandle>,
    /// Canonical selection cache shared with representation lowering.
    pub(crate) selections: &'a mut BTreeMap<String, SelectionHandle>,
    pub(crate) bindings: &'a ScienceBindings,
}

/// Lowers every declared scientific item onto `scene`.
///
/// # Errors
///
/// Returns an invalid-specification error for a grid that contradicts its
/// descriptor, an empty anchor selection, or degenerate measurement and
/// interaction geometry.
pub(crate) fn lower(
    spec: &crate::SceneSpec,
    lowering: &mut SciLowering<'_>,
) -> Result<LoweredScience, Error> {
    let mut lowered = LoweredScience::default();

    for (id, volume) in &spec.volumes {
        if let Some(handle) = lower_volume(lowering.scene, volume, lowering.bindings)? {
            lowered.volumes.push((*id, handle));
        }
    }

    for (id, annotation) in &spec.annotations {
        let anchors = std::slice::from_ref(&annotation.anchor);
        let position = positions(
            lowering.scene,
            lowering.structures,
            lowering.handles,
            lowering.selections,
            anchors,
        )?;
        let owner = owner_of(lowering.scene, lowering.handles, anchors)?;
        // The core annotation path fixes its marker presentation:
        // `Annotation::note` accepts no colour. `AnnotationSpec::color`
        // therefore stays a portable descriptor this lowering does not consume.
        let value = Annotation::note(
            owner,
            AnnotationAnchor::world(position[0])?,
            annotation.text.clone(),
        )?;
        lowered
            .labels
            .push((*id, lowering.scene.add_annotation(value)?));
    }

    for (id, measurement) in &spec.measurements {
        let native = lower_measurement(
            lowering.scene,
            lowering.structures,
            lowering.handles,
            lowering.selections,
            measurement,
        )?;
        lowered
            .measurements
            .push((*id, lowering.scene.add_measurement(native)?));
    }

    for (id, interaction) in &spec.scientific_interactions {
        let ScientificInteractionSpec::Explicit { kind, endpoints } = interaction;
        let points = positions(
            lowering.scene,
            lowering.structures,
            lowering.handles,
            lowering.selections,
            endpoints,
        )?;
        let owner = owner_of(lowering.scene, lowering.handles, endpoints)?;
        let distance = (points[1] - points[0]).length();
        if !distance.is_finite() || distance <= 0.0 {
            return Err(Error::InvalidSpec(
                "interaction endpoints must resolve to distinct finite positions".to_owned(),
            ));
        }
        let edge = InteractionEdge::new(
            owner,
            InteractionAnchor::world(points[0])?,
            InteractionAnchor::world(points[1])?,
            kind.core_kind(),
            InteractionGeometry::new(distance, None)?,
            "molgfx:explicit",
        )?;
        lowered
            .interactions
            .push((*id, lowering.scene.add_interaction(edge)?));
    }

    Ok(lowered)
}

/// Stores nothing for a grid with no runtime binding; the descriptor stays
/// portable and the item is simply unresolved.
fn lower_volume(
    scene: &mut Scene,
    spec: &VolumeSpec,
    bindings: &ScienceBindings,
) -> Result<Option<VolumeHandle>, Error> {
    let Some(binding) = bindings.volume(&spec.source.content_hash) else {
        return Ok(None);
    };
    binding.matches(spec)?;
    Ok(Some(scene.add_volume(binding.native()?)))
}

fn lower_measurement(
    scene: &mut Scene,
    structures: &BTreeMap<StructureId, MolecularSource>,
    handles: &BTreeMap<StructureId, StructureHandle>,
    selections: &mut BTreeMap<String, SelectionHandle>,
    spec: &MeasurementSpec,
) -> Result<Measurement, Error> {
    let (anchors, value, provenance) = match spec {
        MeasurementSpec::Distance { anchors } => {
            let points = positions(scene, structures, handles, selections, anchors)?;
            let value = distance_value(&points)?;
            let world = [
                AnnotationAnchor::world(points[0])?,
                AnnotationAnchor::world(points[1])?,
            ];
            (
                anchors.as_slice(),
                (world.to_vec(), value),
                "molgfx:distance",
            )
        }
        MeasurementSpec::Angle { anchors } => {
            let points = positions(scene, structures, handles, selections, anchors)?;
            let value = angle_value(&points)?;
            let world = [
                AnnotationAnchor::world(points[0])?,
                AnnotationAnchor::world(points[1])?,
                AnnotationAnchor::world(points[2])?,
            ];
            (anchors.as_slice(), (world.to_vec(), value), "molgfx:angle")
        }
        MeasurementSpec::Dihedral { anchors } => {
            let points = positions(scene, structures, handles, selections, anchors)?;
            let value = dihedral_value(&points)?;
            let world = [
                AnnotationAnchor::world(points[0])?,
                AnnotationAnchor::world(points[1])?,
                AnnotationAnchor::world(points[2])?,
                AnnotationAnchor::world(points[3])?,
            ];
            (
                anchors.as_slice(),
                (world.to_vec(), value),
                "molgfx:dihedral",
            )
        }
    };
    let owner = owner_of(scene, handles, anchors)?;
    let (world, value) = value;
    let built = match world.as_slice() {
        [a, b] => Measurement::distance(owner, [*a, *b], value, provenance)?,
        [a, b, c] => Measurement::angle(owner, [*a, *b, *c], value, provenance)?,
        [a, b, c, d] => Measurement::dihedral(owner, [*a, *b, *c, *d], value, provenance)?,
        _ => {
            return Err(Error::InvalidSpec(
                "measurement arity is not two, three or four".to_owned(),
            ));
        }
    };
    Ok(built)
}

/// Owning structure of a scientific item: the first selection anchor's
/// structure, or the scene's first bound structure for world-only anchors,
/// because core ownership drives lifecycle and picking.
fn owner_of(
    scene: &Scene,
    handles: &BTreeMap<StructureId, StructureHandle>,
    anchors: &[Anchor],
) -> Result<StructureHandle, Error> {
    for anchor in anchors {
        if let Anchor::Selection { structure, .. } = anchor
            && let Some(handle) = handles.get(structure)
        {
            return Ok(*handle);
        }
    }
    scene
        .structures()
        .next()
        .map(|(handle, _)| handle)
        .ok_or_else(|| {
            Error::InvalidSpec("a scientific item requires a bound structure".to_owned())
        })
}

fn positions(
    scene: &mut Scene,
    structures: &BTreeMap<StructureId, MolecularSource>,
    handles: &BTreeMap<StructureId, StructureHandle>,
    selections: &mut BTreeMap<String, SelectionHandle>,
    anchors: &[Anchor],
) -> Result<Vec<Vec3>, Error> {
    anchors
        .iter()
        .map(|anchor| anchor_position(scene, structures, handles, selections, anchor))
        .collect()
}

fn anchor_position(
    scene: &mut Scene,
    structures: &BTreeMap<StructureId, MolecularSource>,
    handles: &BTreeMap<StructureId, StructureHandle>,
    selections: &mut BTreeMap<String, SelectionHandle>,
    anchor: &Anchor,
) -> Result<Vec3, Error> {
    match anchor {
        Anchor::World { position } => Ok(Vec3::from_array(*position)),
        Anchor::Selection {
            structure,
            selection,
        } => {
            let handle = selection_handle(
                scene, structures, handles, selections, *structure, selection,
            )?;
            let core = handles.get(structure).copied().ok_or_else(|| {
                Error::InvalidSpec("anchor targets an unbound structure".to_owned())
            })?;
            scene.selection_centroid(handle, core).ok_or_else(|| {
                Error::InvalidSpec("anchor selection resolves to no atoms".to_owned())
            })
        }
    }
}

/// Canonical selection for one query on one structure, shared with the
/// representation cache so equal queries evaluate once.
fn selection_handle(
    scene: &mut Scene,
    structures: &BTreeMap<StructureId, MolecularSource>,
    handles: &BTreeMap<StructureId, StructureHandle>,
    selections: &mut BTreeMap<String, SelectionHandle>,
    structure: StructureId,
    selection: &Selection,
) -> Result<SelectionHandle, Error> {
    let key = format!("{}:{}", structure.get(), selection.stable_hash()?);
    if let Some(handle) = selections.get(&key).copied() {
        return Ok(handle);
    }
    let core = handles
        .get(&structure)
        .copied()
        .ok_or_else(|| Error::InvalidSpec("anchor targets an unbound structure".to_owned()))?;
    let source = structures
        .get(&structure)
        .ok_or_else(|| Error::InvalidSpec("anchor targets an unbound structure".to_owned()))?;
    let rows = source.select(selection.source())?;
    let handle = scene.add_structure_selection(core, rows)?;
    let _ = selections.insert(key, handle);
    Ok(handle)
}

fn distance_value(points: &[Vec3]) -> Result<f32, Error> {
    let value = (points[1] - points[0]).length();
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(Error::InvalidSpec(
            "distance anchors must resolve to distinct finite positions".to_owned(),
        ))
    }
}

fn angle_value(points: &[Vec3]) -> Result<f32, Error> {
    let first = points[0] - points[1];
    let second = points[2] - points[1];
    let scale = first.length() * second.length();
    if !scale.is_finite() || scale <= 0.0 {
        return Err(Error::InvalidSpec(
            "angle anchors must resolve to distinct finite positions".to_owned(),
        ));
    }
    Ok((first.dot(second) / scale)
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees())
}

fn dihedral_value(points: &[Vec3]) -> Result<f32, Error> {
    let first = points[1] - points[0];
    let axis = points[2] - points[1];
    let last = points[3] - points[2];
    let length = axis.length();
    let normal_first = first.cross(axis);
    let normal_last = axis.cross(last);
    if !length.is_finite()
        || length <= 0.0
        || normal_first.length_squared() <= f32::EPSILON
        || normal_last.length_squared() <= f32::EPSILON
    {
        return Err(Error::InvalidSpec(
            "dihedral anchors must resolve to distinct finite positions".to_owned(),
        ));
    }
    let x = normal_first.dot(normal_last);
    let y = normal_first.cross(normal_last).dot(axis) / length;
    Ok(y.atan2(x).to_degrees())
}
