// Optional generic timeline materialization runs before dependent relations.

impl<D: Device> CullPass<D> {
    pub(crate) fn record_point_timelines(
        &self,
        scene: &crate::scene_gpu::GpuScene<D>,
        encoder: &mut D::CommandEncoder,
    ) -> bool {
        let mut dispatches = scene.generic_point_timeline_dispatches().peekable();
        if dispatches.peek().is_none() {
            return false;
        }
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "generic point timeline materialization",
            timestamps: None,
        });
        pass.set_pipeline(&self.point_timeline);
        for dispatch in dispatches {
            pass.set_bind_group(0, dispatch.group, &[]);
            pass.dispatch(dispatch.groups[0], dispatch.groups[1], 1);
        }
        true
    }

    pub(crate) fn record_paged_trajectories(
        &self,
        scene: &crate::scene_gpu::GpuScene<D>,
        encoder: &mut D::CommandEncoder,
    ) -> bool {
        let Some((group, dispatch)) = scene.paged_trajectory_interpolation() else {
            return false;
        };
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "paged trajectory interpolation",
            timestamps: None,
        });
        pass.set_bind_group(0, &scene.group0, &[]);
        pass.set_bind_group(1, group, &[]);
        pass.set_pipeline(&self.paged_trajectory);
        pass.dispatch(dispatch[0], dispatch[1], dispatch[2]);
        true
    }

    pub(crate) fn record_attribute_timelines(
        &self,
        scene: &crate::scene_gpu::GpuScene<D>,
        encoder: &mut D::CommandEncoder,
    ) {
        let mut dispatches = scene.attribute_timeline_dispatches().peekable();
        if dispatches.peek().is_none() {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "attribute timeline materialization",
            timestamps: None,
        });
        pass.set_pipeline(&self.attribute_timeline);
        for dispatch in dispatches {
            pass.set_bind_group(0, dispatch.group, &[]);
            pass.dispatch(dispatch.groups[0], dispatch.groups[1], 1);
        }
    }

    pub(crate) fn record_instance_timelines(
        &self,
        scene: &crate::scene_gpu::GpuScene<D>,
        encoder: &mut D::CommandEncoder,
    ) {
        let mut dispatches = scene.generic_instance_timeline_dispatches().peekable();
        if dispatches.peek().is_none() {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "generic instance timeline materialization",
            timestamps: None,
        });
        pass.set_pipeline(&self.instance_timeline);
        for dispatch in dispatches {
            pass.set_bind_group(0, dispatch.group, &[]);
            pass.dispatch(dispatch.groups[0], dispatch.groups[1], 1);
        }
    }
}
