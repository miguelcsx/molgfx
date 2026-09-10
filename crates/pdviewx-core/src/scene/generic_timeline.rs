//! Two-frame generic streams retained by the scene and sampled on the GPU.

use super::{TemporalAttribute, TemporalInstances, TemporalPoints};
use crate::{
    AttributeHandle, AttributeKind, AttributeValues, CoreError, InstanceBatchHandle,
    PointBatchHandle, Scene,
};
use std::sync::Arc;

/// Borrowed two-frame rigid-transform stream, interpolation alpha and revision.
pub type InstanceFramePair<'a> = (
    &'a Arc<[crate::RigidInstance]>,
    &'a Arc<[crate::RigidInstance]>,
    f32,
    u64,
);

/// Borrowed two-frame point-position stream, interpolation alpha and revision.
pub type PointFramePair<'a> = (&'a Arc<[[f32; 3]]>, &'a Arc<[[f32; 3]]>, f32, u64);

impl Scene {
    pub(crate) fn bind_point_frames(
        &mut self,
        handle: PointBatchHandle,
        start: Arc<[[f32; 3]]>,
        end: Arc<[[f32; 3]]>,
    ) -> Result<(), CoreError> {
        let rows = self
            .point_batch(handle)
            .ok_or(CoreError::StaleHandle)?
            .source_rows()
            .len() as usize;
        if start.len() != rows
            || end.len() != rows
            || start
                .iter()
                .chain(end.iter())
                .any(|position| position.iter().any(|value| !value.is_finite()))
        {
            return Err(invalid(
                "point timeline frames must be finite and match their row domain",
            ));
        }
        self.presentation_revision = self.presentation_revision.wrapping_add(1);
        self.point_timeline.insert(
            handle,
            TemporalPoints {
                start,
                end,
                alpha: 0.0,
                revision: self.presentation_revision,
            },
        );
        self.generic_timeline_binding_revision =
            self.generic_timeline_binding_revision.wrapping_add(1);
        Ok(())
    }

    pub(crate) fn bind_instance_frames(
        &mut self,
        handle: InstanceBatchHandle,
        start: Arc<[crate::RigidInstance]>,
        end: Arc<[crate::RigidInstance]>,
    ) -> Result<(), CoreError> {
        let rows = self
            .instance_batch(handle)
            .ok_or(CoreError::StaleHandle)?
            .source_rows()
            .len() as usize;
        if start.len() != rows || end.len() != rows {
            return Err(invalid(
                "instance timeline frames must match their row domain",
            ));
        }
        self.presentation_revision = self.presentation_revision.wrapping_add(1);
        self.instance_timeline.insert(
            handle,
            TemporalInstances {
                start,
                end,
                alpha: 0.0,
                revision: self.presentation_revision,
            },
        );
        self.generic_timeline_binding_revision =
            self.generic_timeline_binding_revision.wrapping_add(1);
        Ok(())
    }

    pub(crate) fn bind_attribute_frames(
        &mut self,
        handle: AttributeHandle,
        start: AttributeValues,
        end: AttributeValues,
    ) -> Result<(), CoreError> {
        let source = self.attribute(handle).ok_or(CoreError::StaleHandle)?;
        if start.kind() != source.kind()
            || end.kind() != source.kind()
            || start.len() != source.len()
            || end.len() != source.len()
            || matches!(
                source.kind(),
                AttributeKind::Category | AttributeKind::Color
            )
        {
            return Err(invalid(
                "attribute timeline requires source-aligned scalar or vector frames",
            ));
        }
        start.validate()?;
        end.validate()?;
        self.presentation_revision = self.presentation_revision.wrapping_add(1);
        self.attribute_timeline.insert(
            handle,
            TemporalAttribute {
                start,
                end,
                alpha: 0.0,
                revision: self.presentation_revision,
            },
        );
        self.generic_timeline_binding_revision =
            self.generic_timeline_binding_revision.wrapping_add(1);
        Ok(())
    }

    pub(crate) fn sample_instance_frames(
        &mut self,
        handle: InstanceBatchHandle,
        alpha: f32,
    ) -> Result<(), CoreError> {
        self.presentation_revision = self.presentation_revision.wrapping_add(1);
        let stream = self
            .instance_timeline
            .get_mut(&handle)
            .ok_or(CoreError::StaleHandle)?;
        stream.alpha = alpha;
        stream.revision = self.presentation_revision;
        Ok(())
    }

    pub(crate) fn sample_point_frames(
        &mut self,
        handle: PointBatchHandle,
        alpha: f32,
    ) -> Result<(), CoreError> {
        self.presentation_revision = self.presentation_revision.wrapping_add(1);
        let stream = self
            .point_timeline
            .get_mut(&handle)
            .ok_or(CoreError::StaleHandle)?;
        stream.alpha = alpha;
        stream.revision = self.presentation_revision;
        Ok(())
    }

    pub(crate) fn sample_attribute_frames(
        &mut self,
        handle: AttributeHandle,
        alpha: f32,
    ) -> Result<(), CoreError> {
        self.presentation_revision = self.presentation_revision.wrapping_add(1);
        let stream = self
            .attribute_timeline
            .get_mut(&handle)
            .ok_or(CoreError::StaleHandle)?;
        stream.alpha = alpha;
        stream.revision = self.presentation_revision;
        Ok(())
    }

    pub(crate) fn unbind_instance_frames(&mut self, handle: InstanceBatchHandle) {
        if self.instance_timeline.remove(&handle).is_some() {
            self.presentation_revision = self.presentation_revision.wrapping_add(1);
            self.generic_timeline_binding_revision =
                self.generic_timeline_binding_revision.wrapping_add(1);
        }
    }

    pub(crate) fn unbind_point_frames(&mut self, handle: PointBatchHandle) {
        if self.point_timeline.remove(&handle).is_some() {
            self.presentation_revision = self.presentation_revision.wrapping_add(1);
            self.generic_timeline_binding_revision =
                self.generic_timeline_binding_revision.wrapping_add(1);
        }
    }

    pub(crate) fn unbind_attribute_frames(&mut self, handle: AttributeHandle) {
        if self.attribute_timeline.remove(&handle).is_some() {
            self.presentation_revision = self.presentation_revision.wrapping_add(1);
            self.generic_timeline_binding_revision =
                self.generic_timeline_binding_revision.wrapping_add(1);
        }
    }

    /// Active resident transform pair, interpolation alpha and revision.
    #[must_use]
    pub fn instance_frames(&self, handle: InstanceBatchHandle) -> Option<InstanceFramePair<'_>> {
        self.instance_timeline
            .get(&handle)
            .map(|stream| (&stream.start, &stream.end, stream.alpha, stream.revision))
    }

    /// Active resident point-position pair, interpolation alpha and revision.
    #[must_use]
    pub fn point_frames(&self, handle: PointBatchHandle) -> Option<PointFramePair<'_>> {
        self.point_timeline
            .get(&handle)
            .map(|stream| (&stream.start, &stream.end, stream.alpha, stream.revision))
    }

    /// Active resident attribute pair, interpolation alpha and revision.
    #[must_use]
    pub fn attribute_frames(
        &self,
        handle: AttributeHandle,
    ) -> Option<(&AttributeValues, &AttributeValues, f32, u64)> {
        self.attribute_timeline
            .get(&handle)
            .map(|stream| (&stream.start, &stream.end, stream.alpha, stream.revision))
    }

    /// Membership/source-pair revision; sampling alpha does not change it.
    #[must_use]
    pub const fn generic_timeline_binding_revision(&self) -> u64 {
        self.generic_timeline_binding_revision
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidTimeline { reason }
}
