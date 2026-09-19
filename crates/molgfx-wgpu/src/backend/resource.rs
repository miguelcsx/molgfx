//! Accounted wgpu buffers and textures.

use molgfx_gpu::{GpuError, ResourceMemory, TextureDesc};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub(crate) struct ResourceLedger {
    buffers: AtomicU64,
    textures: AtomicU64,
    total: AtomicU64,
    peak: AtomicU64,
    limit: Option<u64>,
}

impl ResourceLedger {
    pub(crate) fn new(limit: Option<u64>) -> Self {
        Self {
            buffers: AtomicU64::new(0),
            textures: AtomicU64::new(0),
            total: AtomicU64::new(0),
            peak: AtomicU64::new(0),
            limit,
        }
    }

    fn reserve(&self, bytes: u64, label: &'static str) -> Result<(), GpuError> {
        let limit = match self.limit {
            Some(limit) => limit,
            None => u64::MAX,
        };
        loop {
            let current = self.total.load(Ordering::Acquire);
            let Some(next) = current.checked_add(bytes) else {
                return Err(GpuError::LimitExceeded {
                    resource: label,
                    limit,
                });
            };
            if self.limit.is_some_and(|limit| next > limit) {
                return Err(GpuError::LimitExceeded {
                    resource: label,
                    limit,
                });
            }
            if self
                .total
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.peak.fetch_max(next, Ordering::Relaxed);
                return Ok(());
            }
        }
    }

    pub(crate) fn reserve_buffer(&self, bytes: u64, label: &'static str) -> Result<(), GpuError> {
        self.reserve(bytes, label)?;
        self.buffers.fetch_add(bytes, Ordering::AcqRel);
        Ok(())
    }

    pub(crate) fn reserve_texture(&self, bytes: u64, label: &'static str) -> Result<(), GpuError> {
        self.reserve(bytes, label)?;
        self.textures.fetch_add(bytes, Ordering::AcqRel);
        Ok(())
    }

    pub(crate) fn usage(&self) -> ResourceMemory {
        ResourceMemory {
            buffer_bytes: self.buffers.load(Ordering::Acquire),
            texture_bytes: self.textures.load(Ordering::Acquire),
            peak_bytes: self.peak.load(Ordering::Acquire),
        }
    }
}

#[derive(Debug)]
/// A wgpu buffer whose requested allocation is released from the device ledger on drop.
pub struct WgpuBuffer {
    pub(crate) raw: wgpu::Buffer,
    bytes: u64,
    ledger: Arc<ResourceLedger>,
}

impl WgpuBuffer {
    pub(crate) fn new(raw: wgpu::Buffer, bytes: u64, ledger: Arc<ResourceLedger>) -> Self {
        Self { raw, bytes, ledger }
    }
}

impl Drop for WgpuBuffer {
    fn drop(&mut self) {
        self.ledger.buffers.fetch_sub(self.bytes, Ordering::AcqRel);
        self.ledger.total.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[derive(Debug)]
/// A wgpu texture whose requested allocation is released from the device ledger on drop.
pub struct WgpuTexture {
    pub(crate) raw: wgpu::Texture,
    bytes: u64,
    ledger: Arc<ResourceLedger>,
}

impl WgpuTexture {
    pub(crate) fn new(raw: wgpu::Texture, bytes: u64, ledger: Arc<ResourceLedger>) -> Self {
        Self { raw, bytes, ledger }
    }
}

impl Drop for WgpuTexture {
    fn drop(&mut self) {
        self.ledger.textures.fetch_sub(self.bytes, Ordering::AcqRel);
        self.ledger.total.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

pub(crate) fn texture_bytes(desc: &TextureDesc) -> Result<u64, GpuError> {
    let bytes_per_texel = match desc.format {
        molgfx_gpu::TextureFormat::R8Unorm => 1,
        molgfx_gpu::TextureFormat::Rg16Float
        | molgfx_gpu::TextureFormat::Rgba8Unorm
        | molgfx_gpu::TextureFormat::Rgba8Snorm
        | molgfx_gpu::TextureFormat::Rgba8UnormSrgb
        | molgfx_gpu::TextureFormat::Bgra8Unorm
        | molgfx_gpu::TextureFormat::Bgra8UnormSrgb
        | molgfx_gpu::TextureFormat::R32Uint
        | molgfx_gpu::TextureFormat::R32Float
        | molgfx_gpu::TextureFormat::Depth32Float => 4,
        molgfx_gpu::TextureFormat::Rgba16Float | molgfx_gpu::TextureFormat::Rg32Float => 8,
    };
    u64::from(desc.width.max(1))
        .checked_mul(u64::from(desc.height.max(1)))
        .and_then(|bytes| bytes.checked_mul(u64::from(desc.depth.max(1))))
        .and_then(|bytes| bytes.checked_mul(bytes_per_texel))
        .ok_or(GpuError::LimitExceeded {
            resource: desc.label,
            limit: u64::MAX,
        })
}

#[cfg(test)]
#[path = "resource_tests.rs"]
mod tests;
