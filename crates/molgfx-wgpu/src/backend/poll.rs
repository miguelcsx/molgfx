//! Non-blocking device progress.

use super::device::WgpuDevice;
use molgfx_gpu::{GpuError, PollOutcome};

impl WgpuDevice {
    /// Runs completed callbacks without waiting for the queue.
    pub(super) fn poll_impl(&self) -> Result<PollOutcome, GpuError> {
        self.errors.check()?;
        let Ok(status) = self.device.poll(wgpu::PollType::Poll) else {
            self.errors.check()?;
            return Err(GpuError::DeviceLost);
        };
        self.errors.check()?;
        Ok(if status.is_queue_empty() {
            PollOutcome::Idle
        } else {
            PollOutcome::Busy
        })
    }
}
