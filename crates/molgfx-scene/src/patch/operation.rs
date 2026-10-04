//! The typed semantic changes a patch can carry.

use crate::appearance::AppearanceRuleSpec;
use crate::color::ColorSpec;
use crate::id::{
    AnnotationId, AppearanceRuleId, EllipsoidId, InteractionId, MeasurementId, PlaneId,
    RepresentationId, StructureId, TrajectoryId, VolumeId,
};
use crate::interop::SceneSnapshot;
use crate::overlay::{
    AnnotationSpec, AssemblySpec, EllipsoidSpec, FitResult, InteractionSpec, MeasurementSpec,
    PlaneSpec, TrajectorySpec, ValidationFinding, VolumeSpec,
};
use crate::representation::Selection;
use crate::representation::form::RepresentationSpec;
use crate::spec::{InteractionChannel, StructureSource};
use crate::{ParameterValue, VisualStyle};
use serde::{Deserialize, Serialize};

/// One typed semantic change.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PatchOperation {
    /// Inserts a categorical grid whose labels are bound separately.
    AddSegmentation {
        /// Complete lifetime identity.
        id: crate::SegmentationId,
        /// Immutable descriptor and independent display styles.
        segmentation: crate::SegmentationSpec,
    },
    /// Removes one categorical grid lifetime.
    RemoveSegmentation {
        /// Exact identity to remove.
        id: crate::SegmentationId,
    },
    /// Replaces only the categorical display table, leaving labels resident.
    SetSegmentStyles {
        /// Exact identity to restyle.
        id: crate::SegmentationId,
        /// Complete replacement display table.
        styles: Vec<crate::SegmentStyle>,
    },
    /// Inserts a molecular source under a new ID.
    ///
    /// The patch announces the structure; coordinates stay in the runtime
    /// binding, so applying this operation requires the caller to have bound
    /// the source for `id` before resolving the scene.
    AddStructure {
        /// Stable identity assigned by the authoring scene.
        id: StructureId,
        /// Portable source descriptor; coordinates remain a runtime binding.
        source: StructureSource,
    },
    /// Inserts a representation under a new ID.
    AddRepresentation {
        /// Stable identity assigned by the authoring scene.
        id: RepresentationId,
        /// Complete immutable representation value.
        representation: RepresentationSpec,
    },
    /// Inserts a density volume whose grid is supplied separately.
    AddVolume {
        /// Stable volume identity.
        id: VolumeId,
        /// Immutable volume metadata.
        volume: VolumeSpec,
    },
    /// Removes a density volume.
    RemoveVolume {
        /// Volume to remove.
        id: VolumeId,
    },
    /// Changes the isovalue used to present a density volume.
    SetVolumeIsovalue {
        /// Volume to update.
        id: VolumeId,
        /// New finite density isovalue.
        isovalue: f32,
    },
    /// Inserts an annotation.
    AddAnnotation {
        /// Stable annotation identity.
        id: AnnotationId,
        /// Immutable annotation value.
        annotation: AnnotationSpec,
    },
    /// Removes an annotation.
    RemoveAnnotation {
        /// Annotation to remove.
        id: AnnotationId,
    },
    /// Inserts a geometric measurement.
    AddMeasurement {
        /// Stable measurement identity.
        id: MeasurementId,
        /// Immutable measurement value.
        measurement: MeasurementSpec,
    },
    /// Removes a measurement.
    RemoveMeasurement {
        /// Measurement to remove.
        id: MeasurementId,
    },
    /// Inserts a caller-supplied overlay interaction.
    AddInteraction {
        /// Stable overlay interaction identity.
        id: InteractionId,
        /// Immutable interaction value.
        interaction: InteractionSpec,
    },
    /// Removes an overlay interaction.
    RemoveInteraction {
        /// Overlay interaction to remove.
        id: InteractionId,
    },
    /// Binds a trajectory descriptor to one structure.
    AddTrajectory {
        /// Stable trajectory identity.
        id: TrajectoryId,
        /// Immutable trajectory metadata.
        trajectory: TrajectorySpec,
    },
    /// Removes a trajectory binding.
    RemoveTrajectory {
        /// Trajectory to remove.
        id: TrajectoryId,
    },
    /// Inserts a per-atom anisotropic-displacement ellipsoid overlay.
    AddEllipsoids {
        /// Stable identity assigned by the authoring scene.
        id: EllipsoidId,
        /// Immutable overlay value.
        spec: EllipsoidSpec,
    },
    /// Removes a per-atom anisotropic-displacement ellipsoid overlay.
    RemoveEllipsoids {
        /// Overlay to remove.
        id: EllipsoidId,
    },
    /// Inserts a caller-authored planar guide.
    AddPlane {
        /// Stable identity assigned by the authoring scene.
        id: PlaneId,
        /// Immutable planar guide value.
        spec: PlaneSpec,
    },
    /// Removes a caller-authored planar guide.
    RemovePlane {
        /// Planar guide to remove.
        id: PlaneId,
    },
    /// Removes a representation.
    RemoveRepresentation {
        /// Representation to remove.
        id: RepresentationId,
    },
    /// Replaces appearance and target atomically.
    ReplaceRepresentation {
        /// Representation to replace.
        id: RepresentationId,
        /// Complete replacement value.
        representation: RepresentationSpec,
    },
    /// Changes the molecular query a representation draws.
    ///
    /// Membership changes, so the representation's geometry is rebuilt; its
    /// appearance, identity and every other representation are untouched.
    SetRepresentationTarget {
        /// Representation to retarget.
        id: RepresentationId,
        /// New `MolFrame` query over the representation's structure.
        target: Selection,
    },
    /// Changes only a representation's base colour.
    ///
    /// Selection-scoped appearance rules still take precedence over it for the
    /// atoms they cover.
    SetColor {
        /// Representation to recolour.
        id: RepresentationId,
        /// New base colour.
        color: ColorSpec,
    },
    /// Adds a selection-scoped colour rule under a new ID.
    AddAppearanceRule {
        /// Stable identity; a higher identity wins where rules overlap.
        id: AppearanceRuleId,
        /// Complete rule value.
        rule: AppearanceRuleSpec,
    },
    /// Replaces a colour rule in place, keeping its precedence.
    ReplaceAppearanceRule {
        /// Rule to replace.
        id: AppearanceRuleId,
        /// Complete replacement value.
        rule: AppearanceRuleSpec,
    },
    /// Removes a colour rule, restoring whatever it covered.
    RemoveAppearanceRule {
        /// Rule to remove.
        id: AppearanceRuleId,
    },
    /// Changes only visibility.
    SetVisibility {
        /// Representation to change.
        id: RepresentationId,
        /// New visibility state.
        visible: bool,
    },
    /// Changes only material opacity.
    SetOpacity {
        /// Representation to change.
        id: RepresentationId,
        /// New opacity.
        opacity: f32,
    },
    /// Replaces the immutable visual expression graph.
    SetVisual {
        /// Representation to change.
        id: RepresentationId,
        /// New style, or `None` to restore built-in appearance.
        visual: Option<VisualStyle>,
    },
    /// Updates one typed visual parameter without changing its program.
    SetParameter {
        /// Representation whose uniform block changes.
        id: RepresentationId,
        /// Stable parameter identity.
        name: Box<str>,
        /// Override value, or `None` to restore the declaration default.
        value: Option<ParameterValue>,
    },
    /// Changes the focus query.
    SetFocus {
        /// New focus query, or `None` to clear focus.
        selection: Option<Selection>,
    },
    /// Replaces one GPU-resident semantic interaction channel.
    SetInteraction {
        /// Channel to update.
        channel: InteractionChannel,
        /// Canonical query, or `None` to clear the channel.
        selection: Option<Selection>,
    },
    /// Replaces renderer-independent view state.
    SetCamera {
        /// New explicit camera, or None to restore automatic framing.
        camera: Option<molgfx_math::Camera>,
    },
    /// Replaces the retained crystallographic assembly and unit-cell state.
    SetAssembly {
        /// Assembly metadata, or None to clear the choice.
        assembly: Option<AssemblySpec>,
    },
    /// Replaces the retained native fitting result.
    SetFitting {
        /// Fitting result, or None to clear it.
        fitting: Option<FitResult>,
    },
    /// Replaces caller-computed validation findings.
    SetValidation {
        /// Findings in deterministic source order.
        findings: Vec<ValidationFinding>,
    },
    /// Replaces live scene state after validating the snapshot and its bindings.
    RestoreSnapshot(Box<SceneSnapshot>),
}
