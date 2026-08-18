//! The presentation surface trait.
//!
//! A lost or outdated surface is a value, never a panic: acquisition
//! returns a typed error, the caller reconfigures and reports the frame as
//! skipped, and the next frame recovers.

use crate::descriptors::TextureFormat;
use crate::device::Device;
use thiserror::Error;

/// Why a frame could not be acquired.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Error)]
#[non_exhaustive]
pub enum SurfaceError {
    /// The surface is lost; reconfigure before the next acquire.
    #[error("surface lost")]
    Lost,
    /// The surface no longer matches the window; reconfigure.
    #[error("surface outdated")]
    Outdated,
    /// Acquisition timed out this frame.
    #[error("surface acquire timed out")]
    Timeout,
    /// The system is out of memory for surface buffers.
    #[error("surface out of memory")]
    OutOfMemory,
}

/// Surface configuration, refreshed on resize.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SurfaceConfig {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The swapchain format the surface chose at open.
    pub format: TextureFormat,
}

/// One acquired swapchain frame.
pub trait SurfaceFrame<D: Device> {
    /// The frame's render-target view.
    fn view(&self) -> &D::TextureView;

    /// Presents the frame.
    fn present(self);
}

/// A presentation surface bound to a window.
pub trait Surface<D: Device>: Sized {
    /// The acquired-frame type.
    type Frame: SurfaceFrame<D>;

    /// (Re)configures for the given size; called on open and on resize.
    fn configure(&mut self, device: &D, config: &SurfaceConfig);

    /// The current configuration.
    fn config(&self) -> &SurfaceConfig;

    /// Acquires the next frame.
    ///
    /// # Errors
    ///
    /// Lost, outdated, timed out, or out of memory — all recoverable by
    /// reconfiguring and skipping the frame.
    fn acquire(&mut self) -> Result<Self::Frame, SurfaceError>;
}
