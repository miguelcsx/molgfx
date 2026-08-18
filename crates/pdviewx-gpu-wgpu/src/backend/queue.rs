//! The wgpu submission queue.

use crate::device::WgpuDevice;
use pdviewx_gpu::{GpuError, TextureWrite};

/// The wgpu queue.
#[derive(Debug)]
pub struct WgpuQueue {
    pub(crate) queue: wgpu::Queue,
}

impl pdviewx_gpu::Queue<WgpuDevice> for WgpuQueue {
    fn write_buffer(&self, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
        self.queue.write_buffer(buffer, offset, data);
    }

    fn write_texture(&self, texture: &wgpu::Texture, write: &TextureWrite<'_>) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
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

    async fn read_buffer_async(
        &self,
        device: &WgpuDevice,
        buffer: &wgpu::Buffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        let end = offset.checked_add(size).ok_or(GpuError::DeviceLost)?;
        let slice = buffer.slice(offset..end);
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
        match receiver.await {
            Ok(Ok(())) => {}
            _ => return Err(GpuError::DeviceLost),
        }
        let data = match slice.get_mapped_range() {
            Ok(view) => view.to_vec(),
            Err(_) => return Err(GpuError::DeviceLost),
        };
        buffer.unmap();
        Ok(data)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_buffer_blocking(
        &self,
        device: &WgpuDevice,
        buffer: &wgpu::Buffer,
        offset: u64,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        pollster::block_on(self.read_buffer_async(device, buffer, offset, size))
    }

    fn timestamp_period(&self) -> f32 {
        self.queue.get_timestamp_period()
    }
}
