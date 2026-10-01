//! Allocating identities for overlay items added one at a time.
//!
//! Each insertion is one atomic patch whose new identity is the next after the
//! highest in its table, so identities are dense, deterministic and never
//! reused while an item holds them.

use super::Scene;
use crate::error::Error;
use crate::spec::{PatchOperation, ScenePatch};

impl Scene {
    pub(crate) fn insert_volume(
        &mut self,
        volume: crate::VolumeSpec,
    ) -> Result<crate::VolumeId, Error> {
        let id = crate::VolumeId(next_id_for(
            self.spec.volumes.last_key_value().map(|(id, _)| id.get()),
            "volume",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddVolume { id, volume }],
        })?;
        Ok(id)
    }

    pub(crate) fn insert_annotation(
        &mut self,
        annotation: crate::AnnotationSpec,
    ) -> Result<crate::AnnotationId, Error> {
        let id = crate::AnnotationId(next_id_for(
            self.spec
                .annotations
                .last_key_value()
                .map(|(id, _)| id.get()),
            "annotation",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddAnnotation { id, annotation }],
        })?;
        Ok(id)
    }

    pub(crate) fn insert_measurement(
        &mut self,
        measurement: crate::MeasurementSpec,
    ) -> Result<crate::MeasurementId, Error> {
        let id = crate::MeasurementId(next_id_for(
            self.spec
                .measurements
                .last_key_value()
                .map(|(id, _)| id.get()),
            "measurement",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddMeasurement { id, measurement }],
        })?;
        Ok(id)
    }

    pub(crate) fn insert_interaction(
        &mut self,
        interaction: crate::InteractionSpec,
    ) -> Result<crate::InteractionId, Error> {
        let id = crate::InteractionId(next_id_for(
            self.spec
                .interactions
                .last_key_value()
                .map(|(id, _)| id.get()),
            "overlay interaction",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddInteraction { id, interaction }],
        })?;
        Ok(id)
    }

    pub(crate) fn insert_ellipsoids(
        &mut self,
        spec: crate::overlay::EllipsoidSpec,
    ) -> Result<crate::EllipsoidId, Error> {
        let id = crate::EllipsoidId(next_id_for(
            self.spec
                .ellipsoids
                .last_key_value()
                .map(|(id, _)| id.get()),
            "ellipsoid overlay",
        )?);
        self.apply(&crate::ScenePatch {
            base_revision: self.spec.revision,
            operations: vec![crate::PatchOperation::AddEllipsoids { id, spec }],
        })?;
        Ok(id)
    }
    pub(crate) fn insert_plane(&mut self, spec: crate::PlaneSpec) -> Result<crate::PlaneId, Error> {
        let id = crate::PlaneId(next_id_for(
            self.spec.planes.last_key_value().map(|(id, _)| id.get()),
            "plane",
        )?);
        self.apply(&crate::ScenePatch {
            base_revision: self.spec.revision,
            operations: vec![crate::PatchOperation::AddPlane { id, spec }],
        })?;
        Ok(id)
    }

    pub(crate) fn insert_trajectory(
        &mut self,
        trajectory: crate::TrajectorySpec,
    ) -> Result<crate::TrajectoryId, Error> {
        let id = crate::TrajectoryId(next_id_for(
            self.spec
                .trajectories
                .last_key_value()
                .map(|(id, _)| id.get()),
            "trajectory",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddTrajectory { id, trajectory }],
        })?;
        Ok(id)
    }
}

fn next_id_for(last: Option<u64>, kind: &str) -> Result<u64, Error> {
    crate::fallback(last, 0)
        .checked_add(1)
        .ok_or_else(|| Error::InvalidSpec(format!("{kind} identity space is exhausted")))
}
