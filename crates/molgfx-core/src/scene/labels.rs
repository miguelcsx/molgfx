//! Revisioned persistent annotation and measurement storage.

use crate::annotation::LabelObject;
use crate::{
    Annotation, AnnotationHandle, CoreError, EntityKind, EntityRef, Measurement, MeasurementHandle,
    Scene,
};

#[cfg(test)]
#[path = "labels_tests.rs"]
mod tests;

impl Scene {
    /// Lowers a caller-computed validation finding into the analytic marker
    /// path used by labels and GPU picking. Detection and severity calibration
    /// remain owned by the caller; this operation only stores the marker.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] when the owner or source entity is
    /// absent, or the marker validation error for malformed style data.
    pub fn add_validation_marker(
        &mut self,
        marker: crate::ValidationMarker,
    ) -> Result<AnnotationHandle, CoreError> {
        let annotation = Annotation::marker(marker.owner, marker.anchor, marker.style)?;
        self.add_annotation(annotation)
    }

    /// Stores one validated annotation.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] if its owner, source entity structure
    /// or region selection is absent.
    pub fn add_annotation(
        &mut self,
        annotation: Annotation,
    ) -> Result<AnnotationHandle, CoreError> {
        self.validate_annotation(&annotation)?;
        self.label_revision = self.label_revision.wrapping_add(1);
        Ok(AnnotationHandle(
            self.labels.insert(LabelObject::Annotation(annotation)),
        ))
    }

    /// Stores one caller-computed typed measurement.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] if its owner or an anchored source
    /// structure is absent.
    pub fn add_measurement(
        &mut self,
        measurement: Measurement,
    ) -> Result<MeasurementHandle, CoreError> {
        self.validate_measurement(&measurement)?;
        self.label_revision = self.label_revision.wrapping_add(1);
        Ok(MeasurementHandle(
            self.labels.insert(LabelObject::Measurement(measurement)),
        ))
    }

    /// Resolves a generation-checked annotation handle.
    #[must_use]
    pub fn annotation(&self, handle: AnnotationHandle) -> Option<&Annotation> {
        match self.labels.get(handle.0)? {
            LabelObject::Annotation(value) => Some(value),
            LabelObject::Measurement(_) => None,
        }
    }

    /// Resolves a generation-checked measurement handle.
    #[must_use]
    pub fn measurement(&self, handle: MeasurementHandle) -> Option<&Measurement> {
        match self.labels.get(handle.0)? {
            LabelObject::Measurement(value) => Some(value),
            LabelObject::Annotation(_) => None,
        }
    }

    /// Mutable annotation access; an edit invalidates the packed label table.
    pub fn annotation_mut(&mut self, handle: AnnotationHandle) -> Option<&mut Annotation> {
        let object = self.labels.get_mut(handle.0)?;
        let LabelObject::Annotation(annotation) = object else {
            return None;
        };
        self.label_revision = self.label_revision.wrapping_add(1);
        Some(annotation)
    }

    /// Mutable measurement access; an edit invalidates the packed label table.
    pub fn measurement_mut(&mut self, handle: MeasurementHandle) -> Option<&mut Measurement> {
        let object = self.labels.get_mut(handle.0)?;
        let LabelObject::Measurement(measurement) = object else {
            return None;
        };
        self.label_revision = self.label_revision.wrapping_add(1);
        Some(measurement)
    }

    /// Removes an annotation and invalidates its handle.
    pub fn remove_annotation(&mut self, handle: AnnotationHandle) -> Option<Annotation> {
        if !matches!(self.labels.get(handle.0), Some(LabelObject::Annotation(_))) {
            return None;
        }
        let LabelObject::Annotation(annotation) = self.labels.remove(handle.0)? else {
            return None;
        };
        self.label_revision = self.label_revision.wrapping_add(1);
        Some(annotation)
    }

    /// Removes a measurement and invalidates its handle.
    pub fn remove_measurement(&mut self, handle: MeasurementHandle) -> Option<Measurement> {
        if !matches!(self.labels.get(handle.0), Some(LabelObject::Measurement(_))) {
            return None;
        }
        let LabelObject::Measurement(measurement) = self.labels.remove(handle.0)? else {
            return None;
        };
        self.label_revision = self.label_revision.wrapping_add(1);
        Some(measurement)
    }

    /// Iterates annotations in stable label-table order.
    pub fn annotations(&self) -> impl Iterator<Item = (AnnotationHandle, &Annotation)> + '_ {
        self.labels
            .iter()
            .filter_map(|(handle, object)| match object {
                LabelObject::Annotation(annotation) => Some((AnnotationHandle(handle), annotation)),
                LabelObject::Measurement(_) => None,
            })
    }

    /// Iterates measurements in stable label-table order.
    pub fn measurements(&self) -> impl Iterator<Item = (MeasurementHandle, &Measurement)> + '_ {
        self.labels
            .iter()
            .filter_map(|(handle, object)| match object {
                LabelObject::Measurement(measurement) => {
                    Some((MeasurementHandle(handle), measurement))
                }
                LabelObject::Annotation(_) => None,
            })
    }

    /// Resolves a picked label row to an annotation in `O(1)`.
    #[must_use]
    pub fn annotation_for_entity(
        &self,
        entity: EntityRef,
    ) -> Option<(AnnotationHandle, &Annotation)> {
        if entity.kind != EntityKind::Label {
            return None;
        }
        let (handle, object) = self.labels.get_index(entity.index)?;
        let LabelObject::Annotation(annotation) = object else {
            return None;
        };
        (annotation.owner() == entity.structure).then_some((AnnotationHandle(handle), annotation))
    }

    /// Resolves a picked label row to a measurement in `O(1)`.
    #[must_use]
    pub fn measurement_for_entity(
        &self,
        entity: EntityRef,
    ) -> Option<(MeasurementHandle, &Measurement)> {
        if entity.kind != EntityKind::Label {
            return None;
        }
        let (handle, object) = self.labels.get_index(entity.index)?;
        let LabelObject::Measurement(measurement) = object else {
            return None;
        };
        (measurement.owner() == entity.structure)
            .then_some((MeasurementHandle(handle), measurement))
    }

    /// Revision key for persistent GPU label and measurement storage.
    #[must_use]
    pub const fn label_revision(&self) -> u64 {
        self.label_revision
    }

    /// Stable label-table row used by annotation picking.
    #[must_use]
    pub fn annotation_row(handle: AnnotationHandle) -> u32 {
        handle.row()
    }

    /// Stable label-table row used by measurement picking.
    #[must_use]
    pub fn measurement_row(handle: MeasurementHandle) -> u32 {
        handle.row()
    }

    fn validate_annotation(&self, annotation: &Annotation) -> Result<(), CoreError> {
        if self.structure(annotation.owner()).is_none()
            || annotation
                .anchor()
                .and_then(crate::AnnotationAnchor::source_entity)
                .is_some_and(|entity| self.structure(entity.structure).is_none())
            || annotation
                .region_selection()
                .is_some_and(|selection| self.selection(selection).is_none())
        {
            return Err(CoreError::StaleHandle);
        }
        Ok(())
    }

    fn validate_measurement(&self, measurement: &Measurement) -> Result<(), CoreError> {
        if self.structure(measurement.owner()).is_none()
            || measurement.anchors().iter().any(|anchor| {
                anchor
                    .source_entity()
                    .is_some_and(|entity| self.structure(entity.structure).is_none())
            })
        {
            return Err(CoreError::StaleHandle);
        }
        Ok(())
    }
}
