//! The browser-facing, serializable form of a device loss.

use molgfx_gpu::DeviceLoss;
use serde::Serialize;

/// A lost device as the viewer receives it: a stable reason name, whether a
/// fresh renderer is expected to succeed, and the backend's message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceLossReport {
    /// Lowercase reason name, `destroyed` or `unknown`.
    pub reason: &'static str,
    /// Whether recreating the renderer from the scene is expected to work.
    pub recoverable: bool,
    /// Backend-supplied detail, possibly empty.
    pub message: String,
}

impl From<&DeviceLoss> for DeviceLossReport {
    fn from(loss: &DeviceLoss) -> Self {
        Self {
            reason: loss.reason.as_str(),
            recoverable: loss.recoverable(),
            message: loss.message.clone(),
        }
    }
}

#[cfg(test)]
#[path = "device_loss_tests.rs"]
mod tests;
