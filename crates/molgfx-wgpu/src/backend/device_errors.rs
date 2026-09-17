//! Bounded asynchronous diagnostics, checked without locking on healthy frames.

use super::device::WgpuDevice;
use molgfx_gpu::GpuError;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

const RUNTIME_PENDING: u8 = 1;
const DEVICE_LOST: u8 = 2;

#[derive(Debug, Default)]
pub(super) struct DeviceErrors {
    status: AtomicU8,
    first: Mutex<Option<String>>,
}

impl DeviceErrors {
    pub(super) fn attach(device: &wgpu::Device) -> Arc<Self> {
        let errors = Arc::new(Self::default());
        let runtime = Arc::clone(&errors);
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            runtime.record(error.to_string());
        }));
        let lost = Arc::clone(&errors);
        device.set_device_lost_callback(move |_, _| {
            lost.status.fetch_or(DEVICE_LOST, Ordering::Release);
        });
        errors
    }

    fn record(&self, detail: String) {
        if let Ok(mut first) = self.first.lock() {
            if first.is_none() {
                *first = Some(detail);
            }
            self.status.fetch_or(RUNTIME_PENDING, Ordering::Release);
        }
    }

    pub(super) fn check(&self) -> Result<(), GpuError> {
        let status = self.status.load(Ordering::Acquire);
        if status & RUNTIME_PENDING != 0 {
            let mut first = self.first.lock().map_err(|_| GpuError::Runtime {
                detail: "GPU diagnostic state is unavailable".to_owned(),
            })?;
            self.status.fetch_and(!RUNTIME_PENDING, Ordering::AcqRel);
            if let Some(detail) = first.take() {
                return Err(GpuError::Runtime { detail });
            }
        }
        if status & DEVICE_LOST != 0 {
            return Err(GpuError::DeviceLost);
        }
        Ok(())
    }
}

impl WgpuDevice {
    /// Captures synchronous compilation diagnostics before leaving native code.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn validated<T>(
        &self,
        label: &'static str,
        create: impl FnOnce() -> T,
    ) -> Result<T, GpuError> {
        self.errors.check()?;
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let value = create();
        let error: Option<wgpu::Error> = pollster::block_on(scope.pop());
        match error {
            None => {
                self.errors.check()?;
                Ok(value)
            }
            Some(error) => Err(GpuError::ShaderCompile {
                label: label.to_owned(),
                detail: error.to_string(),
            }),
        }
    }

    /// Browser errors arrive asynchronously through the device mailbox.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn validated<T>(
        &self,
        _label: &'static str,
        create: impl FnOnce() -> T,
    ) -> Result<T, GpuError> {
        self.errors.check()?;
        let value = create();
        self.errors.check()?;
        Ok(value)
    }
}

#[cfg(test)]
#[path = "device_errors_tests.rs"]
mod tests;
