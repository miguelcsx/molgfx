//! The wgpu submission queue.

use super::device_errors::DeviceErrors;
use super::resource::{WgpuBuffer, WgpuTexture};
use crate::device::WgpuDevice;
use molgfx_gpu::{FenceValue, GpuError, Readback as _, TextureWrite};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The wgpu queue.
#[derive(Debug)]
pub struct WgpuQueue {
    pub(crate) queue: wgpu::Queue,
    pub(crate) next_fence: Arc<AtomicU64>,
    pub(crate) completed_fence: Arc<AtomicU64>,
}

impl molgfx_gpu::Queue<WgpuDevice> for WgpuQueue {
    fn write_buffer(&self, buffer: &WgpuBuffer, offset: u64, data: &[u8]) {
        self.queue.write_buffer(&buffer.raw, offset, data);
    }

    fn write_texture(&self, texture: &WgpuTexture, write: &TextureWrite<'_>) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.raw,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: write.origin[0],
                    y: write.origin[1],
                    z: write.origin[2],
                },
                aspect: wgpu::TextureAspect::All,
            },
            write.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(write.bytes_per_row),
                rows_per_image: Some(write.rows_per_image),
            },
            wgpu::Extent3d {
                width: write.size[0],
                height: write.size[1],
                depth_or_array_layers: write.size[2],
            },
        );
    }

    fn submit(&self, encoder: crate::encoder::WgpuCommandEncoder) {
        self.queue.submit([encoder.encoder.finish()]);
    }

    fn submit_tracked(&self, encoder: crate::encoder::WgpuCommandEncoder) -> FenceValue {
        let fence = self.next_fence.fetch_add(1, Ordering::Relaxed);
        self.queue.submit([encoder.encoder.finish()]);
        let completed = Arc::clone(&self.completed_fence);
        self.queue.on_submitted_work_done(move || {
            completed.fetch_max(fence, Ordering::Release);
        });
        FenceValue(fence)
    }

    fn completed_fence(&self, device: &WgpuDevice) -> Result<FenceValue, GpuError> {
        device.errors.check()?;
        device
            .device
            .poll(wgpu::PollType::Poll)
            .map_err(|_| GpuError::DeviceLost)?;
        device.errors.check()?;
        Ok(FenceValue(self.completed_fence.load(Ordering::Acquire)))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn wait_fence<'a>(
        &'a self,
        device: &'a WgpuDevice,
        fence: FenceValue,
    ) -> impl Future<Output = Result<(), GpuError>> + 'a {
        std::future::ready(self.wait_fence_blocking(device, fence))
    }

    #[cfg(target_arch = "wasm32")]
    async fn wait_fence<'a>(
        &'a self,
        device: &'a WgpuDevice,
        fence: FenceValue,
    ) -> Result<(), GpuError> {
        if self.completed_fence(device)? >= fence {
            return Ok(());
        }
        let (sender, receiver) = futures_channel::oneshot::channel();
        self.queue.on_submitted_work_done(move || {
            let _ = sender.send(());
        });
        receiver.await.map_err(|_| GpuError::DeviceLost)?;
        device.errors.check()?;
        if self.completed_fence.load(Ordering::Acquire) < fence.0 {
            return Err(GpuError::DeviceLost);
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn wait_fence_blocking(&self, device: &WgpuDevice, fence: FenceValue) -> Result<(), GpuError> {
        if self.completed_fence(device)? < fence {
            device
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|_| GpuError::DeviceLost)?;
        }
        device.errors.check()?;
        if self.completed_fence.load(Ordering::Acquire) < fence.0 {
            return Err(GpuError::DeviceLost);
        }
        Ok(())
    }

    async fn read_buffer_async(
        &self,
        device: &WgpuDevice,
        buffer: &WgpuBuffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        self.readback(device, buffer).resolve(offset, size).await
    }

    async fn read_buffer_into_async<'a>(
        &'a self,
        device: &'a WgpuDevice,
        buffer: &'a WgpuBuffer,
        offset: u64,
        size: u64,
        output: &'a mut [u8],
    ) -> Result<(), GpuError> {
        self.readback(device, buffer)
            .resolve_into(offset, size, output)
            .await
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_buffer_into_blocking(
        &self,
        device: &WgpuDevice,
        buffer: &WgpuBuffer,
        offset: u64,
        size: u64,
        output: &mut [u8],
    ) -> Result<(), GpuError> {
        pollster::block_on(
            self.readback(device, buffer)
                .resolve_into(offset, size, output),
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_buffer_blocking(
        &self,
        device: &WgpuDevice,
        buffer: &WgpuBuffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        pollster::block_on(self.readback(device, buffer).resolve(offset, size))
    }

    fn readback(&self, device: &WgpuDevice, buffer: &WgpuBuffer) -> WgpuReadback {
        WgpuReadback {
            device: device.device.clone(),
            errors: Arc::clone(&device.errors),
            buffer: buffer.raw.clone(),
        }
    }

    fn timestamp_period(&self) -> f32 {
        self.queue.get_timestamp_period()
    }
}

/// A readback that owns its buffer and device handles, so the wait borrows
/// neither the queue nor the device that recorded the copy.
#[derive(Debug)]
pub struct WgpuReadback {
    device: wgpu::Device,
    errors: Arc<DeviceErrors>,
    buffer: wgpu::Buffer,
}

impl molgfx_gpu::Readback for WgpuReadback {
    async fn resolve(&self, offset: u64, size: u64) -> Result<Vec<u8>, GpuError> {
        self.with_mapped_bytes(offset, size, None, <[u8]>::to_vec)
            .await
    }

    async fn resolve_into<'a>(
        &'a self,
        offset: u64,
        size: u64,
        output: &'a mut [u8],
    ) -> Result<(), GpuError> {
        self.with_mapped_bytes(offset, size, Some(output.len()), |bytes| {
            output[..bytes.len()].copy_from_slice(bytes);
        })
        .await
    }
}

impl WgpuReadback {
    async fn with_mapped_bytes<T>(
        &self,
        offset: u64,
        size: u64,
        output_capacity: Option<usize>,
        copy: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, GpuError> {
        self.errors.check()?;
        let end = offset.checked_add(size).ok_or_else(|| GpuError::Runtime {
            detail: "GPU readback range overflows its address space".to_owned(),
        })?;
        if end > self.buffer.size()
            || size == 0
            || !offset.is_multiple_of(wgpu::MAP_ALIGNMENT)
            || !size.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
        {
            return Err(GpuError::Runtime {
                detail: "GPU readback range is empty, unaligned, or exceeds its buffer".to_owned(),
            });
        }
        let length = usize::try_from(size).map_err(|_| GpuError::Runtime {
            detail: "GPU readback range exceeds the host address space".to_owned(),
        })?;
        if output_capacity.is_some_and(|capacity| capacity < length) {
            return Err(GpuError::Runtime {
                detail: "GPU readback output is shorter than the requested range".to_owned(),
            });
        }
        if !self.buffer.usage().contains(wgpu::BufferUsages::MAP_READ) {
            return Err(GpuError::Runtime {
                detail: "GPU readback buffer does not permit host reads".to_owned(),
            });
        }
        let slice = self.buffer.slice(offset..end);
        let (sender, receiver) = futures_channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        // This guard also cancels an outstanding map if the future is dropped.
        // It precedes the view so the view always drops before unmapping.
        let _unmap = UnmapOnDrop(&self.buffer);
        #[cfg(not(target_arch = "wasm32"))]
        let poll_type = wgpu::PollType::wait_indefinitely();
        #[cfg(target_arch = "wasm32")]
        let poll_type = wgpu::PollType::Poll;
        if self.device.poll(poll_type).is_err() {
            self.errors.check()?;
            return Err(GpuError::DeviceLost);
        }
        let mapped = receiver.await;
        self.errors.check()?;
        mapped
            .map_err(|error| GpuError::Runtime {
                detail: format!("GPU readback completion failed: {error}"),
            })?
            .map_err(|error| GpuError::Runtime {
                detail: format!("GPU readback mapping failed: {error}"),
            })?;
        let view = slice
            .get_mapped_range()
            .map_err(|error| GpuError::Runtime {
                detail: format!("GPU mapped readback is unavailable: {error}"),
            })?;
        Ok(copy(&view))
    }
}

struct UnmapOnDrop<'a>(&'a wgpu::Buffer);

impl Drop for UnmapOnDrop<'_> {
    fn drop(&mut self) {
        self.0.unmap();
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "queue_tests.rs"]
mod tests;
