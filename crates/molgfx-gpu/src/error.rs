//! Typed GPU errors with stable registry codes.
//!
//! A lost device or surface is a recoverable state the caller handles as a
//! value; nothing in the engine panics on a condition a caller could hit.

use crate::surface::SurfaceError;
use thiserror::Error;

/// Everything that can go wrong opening or driving a device.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GpuError {
    /// No adapter satisfied the device request.
    #[error("no compatible GPU adapter found: {detail}")]
    NoAdapter {
        /// Backend selection diagnostics supplied by the active HAL.
        detail: String,
    },

    /// A window or canvas could not be converted into a presentation surface.
    #[error("GPU presentation surface creation failed: {detail}")]
    SurfaceCreation {
        /// Backend-reported creation failure.
        detail: String,
    },

    /// An adapter was found but opening its logical device failed.
    #[error("GPU device request failed: {detail}")]
    DeviceRequest {
        /// Backend-reported device negotiation failure.
        detail: String,
    },

    /// The device was lost; recreate and retry.
    #[error("device lost")]
    DeviceLost,

    /// The presentation surface is unavailable; reconfigure and skip the
    /// frame.
    #[error("surface unavailable")]
    Surface(#[from] SurfaceError),

    /// A requested capability is unavailable and no fallback applies.
    #[error("capability {name:?} unavailable")]
    Capability {
        /// The capability that was required.
        name: &'static str,
    },

    /// A requested resource exceeds a reported portable device limit.
    #[error("resource {resource:?} exceeds device limit {limit}")]
    LimitExceeded {
        /// Resource class that exceeded the limit.
        resource: &'static str,
        /// Reported device ceiling.
        limit: u64,
    },

    /// A shader failed to compile on this backend.
    #[error("shader compilation failed in {label:?}: {detail}")]
    ShaderCompile {
        /// Which shader module.
        label: String,
        /// Backend-reported detail, including stage and location where
        /// available.
        detail: String,
    },

    /// A GPU operation failed validation or exhausted backend resources.
    #[error("GPU runtime operation failed: {detail}")]
    Runtime {
        /// The first backend diagnostic, retained before secondary failures.
        detail: String,
    },
}

impl GpuError {
    /// The stable registry code for this condition.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoAdapter { .. } => "MOLGFX-E0001",
            Self::SurfaceCreation { .. } => "MOLGFX-E0004",
            Self::DeviceRequest { .. } => "MOLGFX-E0005",
            Self::DeviceLost => "MOLGFX-E0002",
            Self::Surface(_) => "MOLGFX-E0003",
            Self::Capability { .. } => "MOLGFX-E0010",
            Self::LimitExceeded { .. } => "MOLGFX-E0011",
            Self::ShaderCompile { .. } => "MOLGFX-E0060",
            Self::Runtime { .. } => "MOLGFX-E0061",
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
