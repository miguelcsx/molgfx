//! The submission queue trait.

use crate::device::Device;
use crate::error::GpuError;
use crate::{FenceValue, TextureWrite};
use std::future::Future;

/// A detached readback over one buffer.
///
/// The handle owns everything it needs to await a mapped range, so a caller
/// can release every borrow of the renderer that submitted the copy before
/// the wait begins. On the browser's single JavaScript thread that is what
/// lets a frame render while a pick is still resolving its readback.
pub trait Readback: std::fmt::Debug {
    /// Awaits the mapped range and returns its bytes.
    ///
    /// # Errors
    ///
    /// The device was lost, or the buffer was not readable.
    fn resolve(
        &self,
        offset: u64,
        size: u64,
    ) -> impl Future<Output = Result<Vec<u8>, GpuError>> + '_;

    /// Copies a mapped range into caller-owned scratch without allocating bytes.
    /// Only the first `size` bytes are written; the remaining output is unchanged.
    /// The future borrows this detached handle and the scratch, not the renderer.
    ///
    /// # Errors
    /// Returns device or mapping errors, or an invalid, unaligned, overflowing
    /// range or output shorter than `size`, before writing any output.
    fn resolve_into<'a>(
        &'a self,
        offset: u64,
        size: u64,
        output: &'a mut [u8],
    ) -> impl Future<Output = Result<(), GpuError>> + 'a;
}

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

    /// Submits work whose completion must gate resource residency.
    fn submit_tracked(&self, encoder: D::CommandEncoder) -> FenceValue;

    /// Polls the backend and returns the greatest submission known complete.
    ///
    /// # Errors
    ///
    /// Returns device loss when completion status can no longer be queried.
    fn completed_fence(&self, device: &D) -> Result<FenceValue, GpuError>;

    /// Awaits a tracked submission without mapping a pixel or timestamp buffer.
    ///
    /// # Errors
    /// Returns device loss or a failed completion observation.
    fn wait_fence<'a>(
        &'a self,
        device: &'a D,
        fence: FenceValue,
    ) -> impl Future<Output = Result<(), GpuError>> + 'a;

    /// Native blocking completion wait, without readback.
    ///
    /// # Errors
    /// Returns device loss or a failed completion observation.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    fn wait_fence_blocking(&self, device: &D, fence: FenceValue) -> Result<(), GpuError>;

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

    /// Resolves a buffer range into reserved host scratch without allocating bytes.
    /// Only the first `size` bytes are written; the remaining output is unchanged.
    ///
    /// # Errors
    /// Returns the same errors as [`Readback::resolve_into`].
    fn read_buffer_into_async<'a>(
        &'a self,
        device: &'a D,
        buffer: &'a D::Buffer,
        offset: u64,
        size: u64,
        output: &'a mut [u8],
    ) -> impl Future<Output = Result<(), GpuError>> + 'a;

    /// Detaches a readback handle over one buffer.
    ///
    /// The returned handle borrows nothing, so the caller is free to use the
    /// device and queue while a [`Readback::resolve`] is still awaiting.
    fn readback(&self, device: &D, buffer: &D::Buffer) -> D::Readback;

    /// Native convenience that waits for a mapped range. Browser callers use
    /// [`Self::read_buffer_async`].
    ///
    /// # Errors
    ///
    /// The device was lost, or the buffer was not readable.
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    fn read_buffer_blocking(
        &self,
        device: &D,
        buffer: &D::Buffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError>;

    /// Native blocking read into reserved host scratch.
    ///
    /// # Errors
    /// Returns the same errors as [`Readback::resolve_into`].
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    fn read_buffer_into_blocking(
        &self,
        device: &D,
        buffer: &D::Buffer,
        offset: u64,
        size: u64,
        output: &mut [u8],
    ) -> Result<(), GpuError>;

    /// Nanoseconds represented by one timestamp-query tick.
    fn timestamp_period(&self) -> f32;
}
