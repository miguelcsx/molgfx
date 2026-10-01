//! Planning the commands that add overlay items: labels, measurements and
//! caller-supplied interactions, plus the retained domain metadata.

use super::{Planner, scene_error};
use crate::error::{CommandError, ErrorKind};
use crate::ir::{MeasureKind, Name, QueryText};
use molgfx_scene::{
    Anchor, AnnotationId, AnnotationSpec, Color, InteractionId, InteractionSpec, MeasurementId,
    MeasurementSpec, PatchOperation, PlaneId, PlaneSpec, StructureId, VolumeId, VolumeSpec,
};

impl Planner<'_> {
    pub(super) fn domain(&mut self, operation: PatchOperation) -> Result<(), CommandError> {
        self.transaction
            .stage(operation)
            .map_err(|error| scene_error(&error))
    }
    /// The centroid anchor of `query` in `structure`.
    pub(super) fn anchor(
        &self,
        structure: StructureId,
        query: &QueryText,
    ) -> Result<Anchor, CommandError> {
        Ok(Anchor::Selection {
            structure,
            selection: self.resolver().selection(query)?,
        })
    }

    pub(super) fn label(
        &mut self,
        text: &str,
        target: &QueryText,
        structure: Option<&Name>,
    ) -> Result<(), CommandError> {
        let structure = self.structure(structure)?;
        let anchor = self.anchor(structure, target)?;
        let id = AnnotationId::new(next_identity(
            self.transaction
                .spec()
                .annotations
                .keys()
                .next_back()
                .map(|id| id.get()),
            "annotation",
        )?);
        self.transaction
            .stage(PatchOperation::AddAnnotation {
                id,
                annotation: AnnotationSpec {
                    anchor,
                    text: text.into(),
                    color: Color::rgb(255, 255, 255),
                },
            })
            .map_err(|error| scene_error(&error))
    }

    pub(super) fn measure(
        &mut self,
        kind: MeasureKind,
        points: &[QueryText],
        structure: Option<&Name>,
    ) -> Result<(), CommandError> {
        let structure = self.structure(structure)?;
        let mut anchors = Vec::with_capacity(points.len());
        for point in points {
            anchors.push(self.anchor(structure, point)?);
        }
        let measurement = match (kind, <[Anchor; 4]>::try_from(anchors.clone())) {
            (MeasureKind::Dihedral, Ok(anchors)) => MeasurementSpec::Dihedral { anchors },
            _ => match (kind, anchors.as_slice()) {
                (MeasureKind::Distance, [a, b]) => MeasurementSpec::Distance {
                    anchors: [a.clone(), b.clone()],
                },
                (MeasureKind::Angle, [a, b, c]) => MeasurementSpec::Angle {
                    anchors: [a.clone(), b.clone(), c.clone()],
                },
                _ => {
                    return Err(CommandError::new(
                        ErrorKind::Syntax,
                        format!("{} takes {} points", kind.name(), kind.arity()),
                    ));
                }
            },
        };
        let id = MeasurementId::new(next_identity(
            self.transaction
                .spec()
                .measurements
                .keys()
                .next_back()
                .map(|id| id.get()),
            "measurement",
        )?);
        self.transaction
            .stage(PatchOperation::AddMeasurement { id, measurement })
            .map_err(|error| scene_error(&error))
    }

    pub(super) fn interaction(
        &mut self,
        interaction: &InteractionSpec,
    ) -> Result<(), CommandError> {
        // The command path accepts only the API's explicit variant. The API
        // validates anchors against the scene when this patch is staged; no
        // chemistry or MolFrame analysis occurs here.
        if !matches!(interaction, InteractionSpec::Explicit { .. }) {
            return Err(CommandError::new(
                ErrorKind::Scene,
                "overlay interactions must be explicit caller-supplied values",
            ));
        }
        let next = self
            .transaction
            .spec()
            .interactions
            .keys()
            .next_back()
            .map_or(Some(1), |id| id.get().checked_add(1));
        let Some(next) = next else {
            return Err(CommandError::new(
                ErrorKind::Scene,
                "overlay interaction identity space is exhausted",
            ));
        };
        let id = InteractionId::new(next);
        self.transaction
            .stage(PatchOperation::AddInteraction {
                id,
                interaction: interaction.clone(),
            })
            .map_err(|error| scene_error(&error))
    }
    pub(super) fn plane(&mut self, plane: PlaneSpec) -> Result<(), CommandError> {
        let id = PlaneId::new(next_identity(
            self.transaction
                .spec()
                .planes
                .keys()
                .next_back()
                .map(|id| id.get()),
            "plane",
        )?);
        self.transaction
            .stage(PatchOperation::AddPlane { id, spec: plane })
            .map_err(|error| scene_error(&error))
    }

    pub(super) fn volume(&mut self, volume: VolumeSpec) -> Result<(), CommandError> {
        let id = VolumeId::new(next_identity(
            self.transaction
                .spec()
                .volumes
                .keys()
                .next_back()
                .map(|id| id.get()),
            "volume",
        )?);
        self.transaction
            .stage(PatchOperation::AddVolume { id, volume })
            .map_err(|error| scene_error(&error))
    }
}

/// The identity after `last`, or the first when there is none.
fn next_identity(last: Option<u64>, noun: &str) -> Result<u64, CommandError> {
    last.map_or(Some(1), |last| last.checked_add(1))
        .ok_or_else(|| {
            CommandError::new(
                ErrorKind::Scene,
                format!("{noun} identity space is exhausted"),
            )
        })
}
