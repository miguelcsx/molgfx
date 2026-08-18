//! Trajectory compute recording for resident coordinate frames.

use super::GpuScene;
use crate::passes::TrajectoryPass;
use pdviewx_gpu::{CommandEncoder, ComputePassDesc, Device};

impl<D: Device> GpuScene<D> {
    /// Records dirty coordinate interpolation into the frame encoder.
    pub fn record_trajectories(
        &mut self,
        encoder: &mut D::CommandEncoder,
        trajectory: &TrajectoryPass<D>,
        timestamps: Option<pdviewx_gpu::TimestampWrites<'_, D>>,
    ) -> bool {
        if !self
            .structures
            .iter()
            .any(super::GpuStructure::trajectory_dirty)
        {
            return false;
        }
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "trajectory interpolation",
            timestamps,
        });
        for structure in &mut self.structures {
            structure.record_trajectory(&mut pass, &trajectory.pipeline);
        }
        true
    }
}
