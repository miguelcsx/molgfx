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
    #[error("no compatible GPU adapter found")]
    NoAdapter,

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
}

impl GpuError {
    /// The stable registry code for this condition.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoAdapter => "PDVIEWX-E0001",
            Self::DeviceLost => "PDVIEWX-E0002",
            Self::Surface(_) => "PDVIEWX-E0003",
            Self::Capability { .. } => "PDVIEWX-E0010",
            Self::LimitExceeded { .. } => "PDVIEWX-E0011",
            Self::ShaderCompile { .. } => "PDVIEWX-E0060",
        }
    }
}
