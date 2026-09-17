//! The wgpu submission queue.

use super::resource::{WgpuBuffer, WgpuTexture};
use crate::device::WgpuDevice;
use molgfx_gpu::{FenceValue, GpuError, TextureWrite};
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

    async fn read_buffer_async(
        &self,
        device: &WgpuDevice,
        buffer: &WgpuBuffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        device.errors.check()?;
        let end = offset.checked_add(size).ok_or_else(|| GpuError::Runtime {
            detail: "GPU readback range overflows its address space".to_owned(),
        })?;
        let slice = buffer.raw.slice(offset..end);
        let (sender, receiver) = futures_channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        #[cfg(not(target_arch = "wasm32"))]
        let poll_type = wgpu::PollType::wait_indefinitely();
        #[cfg(target_arch = "wasm32")]
        let poll_type = wgpu::PollType::Poll;
        device
            .device
            .poll(poll_type)
            .map_err(|_| GpuError::DeviceLost)?;
        let mapped = receiver.await;
        if let Err(error) = device.errors.check() {
            buffer.raw.unmap();
            return Err(error);
        }
        match mapped {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                return Err(GpuError::Runtime {
                    detail: format!("GPU readback mapping failed: {error}"),
                });
            }
            Err(error) => {
                return Err(GpuError::Runtime {
                    detail: format!("GPU readback completion failed: {error}"),
                });
            }
        }
        let data = match slice.get_mapped_range() {
            Ok(view) => view.to_vec(),
            Err(error) => {
                buffer.raw.unmap();
                return Err(GpuError::Runtime {
                    detail: format!("GPU mapped readback is unavailable: {error}"),
                });
            }
        };
        buffer.raw.unmap();
        Ok(data)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_buffer_blocking(
        &self,
        device: &WgpuDevice,
        buffer: &WgpuBuffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        pollster::block_on(self.read_buffer_async(device, buffer, offset, size))
    }

    fn timestamp_period(&self) -> f32 {
        self.queue.get_timestamp_period()
    }
}
