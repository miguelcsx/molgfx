//! Cross-object validation for scientific semantic values.

use super::{
    Anchor, AnnotationSpec, DataSource, MeasurementSpec, ScientificInteractionSpec, TrajectorySpec,
    VolumeSpec,
};
use crate::Error;

impl DataSource {
    pub(super) fn validate(&self) -> Result<(), Error> {
        if self.content_hash.is_empty() && self.uri.as_deref().is_none_or(str::is_empty) {
            return Err(Error::InvalidSpec(
                "a data source needs a content hash or URI".to_owned(),
            ));
        }
        Ok(())
    }
}

impl Anchor {
    fn validate(&self, scene: &crate::SceneSpec) -> Result<(), Error> {
        match self {
            Self::World { position } if position.iter().all(|value| value.is_finite()) => Ok(()),
            Self::World { .. } => Err(Error::InvalidSpec(
                "world anchors must contain finite coordinates".to_owned(),
            )),
            Self::Selection {
                structure,
                selection,
            } => {
                if !scene.structures.contains_key(structure) {
                    return Err(Error::InvalidSpec(
                        "anchor targets an unknown structure".to_owned(),
                    ));
                }
                let _ = selection.fingerprint()?;
                Ok(())
            }
        }
    }
}

impl VolumeSpec {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        self.source.validate()?;
        let spacing_is_valid = self
            .spacing
            .iter()
            .all(|value| value.is_finite() && *value > 0.0);
        if self.dimensions.iter().any(|dimension| *dimension < 2)
            || !spacing_is_valid
            || !self.origin.iter().all(|value| value.is_finite())
            || !self.isovalue.is_finite()
        {
            return Err(Error::InvalidSpec(
                "invalid density volume metadata".to_owned(),
            ));
        }
        Ok(())
    }
}

impl AnnotationSpec {
    pub(crate) fn validate(&self, scene: &crate::SceneSpec) -> Result<(), Error> {
        if self.text.is_empty() {
            return Err(Error::InvalidSpec(
                "annotation text cannot be empty".to_owned(),
            ));
        }
        self.anchor.validate(scene)
    }
}

impl MeasurementSpec {
    pub(crate) fn validate(&self, scene: &crate::SceneSpec) -> Result<(), Error> {
        match self {
            Self::Distance { anchors } => anchors.iter().try_for_each(|a| a.validate(scene)),
            Self::Angle { anchors } => anchors.iter().try_for_each(|a| a.validate(scene)),
            Self::Dihedral { anchors } => anchors.iter().try_for_each(|a| a.validate(scene)),
        }
    }
}

impl ScientificInteractionSpec {
    pub(crate) fn validate(&self, scene: &crate::SceneSpec) -> Result<(), Error> {
        match self {
            Self::Explicit { endpoints, .. } => endpoints
                .iter()
                .try_for_each(|anchor| anchor.validate(scene)),
        }
    }
}

impl TrajectorySpec {
    pub(crate) fn validate(&self, scene: &crate::SceneSpec) -> Result<(), Error> {
        self.source.validate()?;
        if !scene.structures.contains_key(&self.structure) || self.frame_count == 0 {
            return Err(Error::InvalidSpec(
                "trajectory needs a known structure and at least one frame".to_owned(),
            ));
        }
        if self
            .time_step
            .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(Error::InvalidSpec(
                "trajectory time step must be finite and positive".to_owned(),
            ));
        }
        Ok(())
    }
}
