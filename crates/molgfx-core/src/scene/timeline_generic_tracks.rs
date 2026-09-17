impl Timeline {
    /// Binds two source-aligned point frames for GPU interpolation.
    ///
    /// # Errors
    ///
    /// Returns a typed error for stale, non-finite, mismatched or duplicate input.
    pub fn bind_points(
        &mut self,
        scene: &mut Scene,
        batch: crate::PointBatchHandle,
        start: Arc<[[f32; 3]]>,
        end: Arc<[[f32; 3]]>,
        warp: TimeWarp,
    ) -> Result<TimelineTrackHandle, CoreError> {
        if self
            .tracks
            .iter()
            .any(|(_, track)| track.targets_points(batch))
        {
            return Err(invalid("a point batch may have only one timeline track"));
        }
        scene.bind_point_frames(batch, start, end)?;
        Ok(TimelineTrackHandle(
            self.tracks.insert(TimelineTrack::Points { batch, warp }),
        ))
    }

    /// Binds two source-aligned rigid-transform frames for GPU interpolation.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale batch, mismatched row counts or a
    /// second timeline targeting the same batch.
    pub fn bind_instances(
        &mut self,
        scene: &mut Scene,
        batch: InstanceBatchHandle,
        start: Arc<[crate::RigidInstance]>,
        end: Arc<[crate::RigidInstance]>,
        warp: TimeWarp,
    ) -> Result<TimelineTrackHandle, CoreError> {
        if self
            .tracks
            .iter()
            .any(|(_, track)| track.targets_instances(batch))
        {
            return Err(invalid(
                "an instance batch may have only one timeline track",
            ));
        }
        scene.bind_instance_frames(batch, start, end)?;
        Ok(TimelineTrackHandle(
            self.tracks.insert(TimelineTrack::Instances { batch, warp }),
        ))
    }

    /// Binds two scalar or vector attribute frames for GPU interpolation.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale attribute, incompatible frames or a
    /// second timeline targeting the same attribute.
    pub fn bind_attribute(
        &mut self,
        scene: &mut Scene,
        attribute: AttributeHandle,
        start: AttributeValues,
        end: AttributeValues,
        warp: TimeWarp,
    ) -> Result<TimelineTrackHandle, CoreError> {
        if self
            .tracks
            .iter()
            .any(|(_, track)| track.targets_attribute(attribute))
        {
            return Err(invalid("an attribute may have only one timeline track"));
        }
        scene.bind_attribute_frames(attribute, start, end)?;
        Ok(TimelineTrackHandle(
            self.tracks
                .insert(TimelineTrack::Attribute { attribute, warp }),
        ))
    }

    /// Removes a track and releases any generic resident frame pair it owns.
    pub fn unbind(&mut self, scene: &mut Scene, handle: TimelineTrackHandle) -> bool {
        let Some(track) = self.tracks.remove(handle.0) else {
            return false;
        };
        match track {
            TimelineTrack::Points { batch, .. } => scene.unbind_point_frames(batch),
            TimelineTrack::Instances { batch, .. } => scene.unbind_instance_frames(batch),
            TimelineTrack::Attribute { attribute, .. } => {
                scene.unbind_attribute_frames(attribute);
            }
            TimelineTrack::Trajectory { .. }
            | TimelineTrack::BondTopology { .. }
            | TimelineTrack::AtomProperty { .. } => {}
        }
        self.revision = self.revision.wrapping_add(1);
        true
    }
}
