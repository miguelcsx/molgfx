//! The submission queue trait.

use crate::TextureWrite;
use crate::device::Device;
use crate::error::GpuError;
use std::future::Future;

/// Uploads and submission. One submission per frame is the discipline the
/// engine holds; the trait does not enforce it, the render loop does.
pub trait Queue<D: Device> {
    /// Writes bytes into a buffer at an offset. The source is borrowed for
    /// the duration of the call — an upload from a caller's slice is
    /// copy-free on the host side.
    fn write_buffer(&self, buffer: &D::Buffer, offset: u64, data: &[u8]);

    /// Uploads a borrowed, tightly described region into a texture.
    fn write_texture(&self, texture: &D::Texture, write: &TextureWrite<'_>);

    /// Submits one encoder's recorded work.
    fn submit(&self, encoder: D::CommandEncoder);

    /// Resolves a mapped buffer range without blocking the browser event loop.
    /// Off the frame path only: golden-image capture, picking and export.
    ///
    /// # Errors
    ///
    /// The device was lost, or the buffer was not readable.
    fn read_buffer_async<'a>(
        &'a self,
        device: &'a D,
        buffer: &'a D::Buffer,
        offset: u64,
        size: u64,
    ) -> impl Future<Output = Result<Vec<u8>, GpuError>> + 'a;

    /// Native convenience that waits for a mapped range. Browser callers use
    /// [`Self::read_buffer_async`].
    ///
    /// # Errors
    ///
    /// The device was lost, or the buffer was not readable.
    #[cfg(not(target_arch = "wasm32"))]
    fn read_buffer_blocking(
        &self,
        device: &D,
        buffer: &D::Buffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError>;

    /// Nanoseconds represented by one timestamp-query tick.
    fn timestamp_period(&self) -> f32;
}
