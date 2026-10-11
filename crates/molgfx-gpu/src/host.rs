//! Host-supplied GPU contexts and render targets.
//!
//! The one place the abstraction admits that a process may already own a GPU
//! context: an embedding application (a desktop shell, a compositor, another
//! engine) hands the renderer its device and queue, and its textures, instead
//! of letting the renderer open its own. Everything here is an interface; the
//! backend decides what its native handles are.

use crate::{Device, GpuError, Opened, TextureFormat};

/// How a host-supplied device is adopted.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostDeviceDesc {
    /// Optional hard ceiling for live buffer and texture bytes the renderer
    /// allocates on the host's device. The host's own resources are not
    /// counted.
    pub resource_memory_limit_bytes: Option<u64>,
    /// Whether the renderer installs its own asynchronous error and device-loss
    /// handlers on the device. Handlers are per device, so installing them
    /// replaces the host's. The default leaves them alone.
    pub observe_device_errors: bool,
}

/// What importing a host texture established about it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ImportedTarget {
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// The host texture's own format.
    pub texture_format: TextureFormat,
    /// The format the renderer writes through: the linear member of the
    /// texture's format family.
    pub render_format: TextureFormat,
}

impl TextureFormat {
    /// The linear member of an sRGB format family; every other format is its
    /// own. The renderer writes display-encoded values through this view, so
    /// an sRGB texture holds the same bytes a linear one does and samples
    /// back correctly.
    #[must_use]
    pub const fn without_srgb(self) -> Self {
        match self {
            Self::Rgba8UnormSrgb => Self::Rgba8Unorm,
            Self::Bgra8UnormSrgb => Self::Bgra8Unorm,
            other => other,
        }
    }

    /// Whether this format may be the renderer's output format on a host
    /// target.
    #[must_use]
    pub const fn is_host_output(self) -> bool {
        matches!(self, Self::Rgba8Unorm | Self::Bgra8Unorm)
    }
}

/// A device that can adopt a host's GPU context and render into its textures.
pub trait HostDevice: Device {
    /// The backend-native handles the host owns and lends: the adapter,
    /// device and queue it already opened.
    type Context;
    /// A backend-native texture handle. Cloning shares the underlying
    /// texture; the host's clone keeps it alive for as long as it likes.
    type ExternalTexture: Clone + std::fmt::Debug;

    /// Adopts the host's context instead of opening a device.
    ///
    /// # Errors
    ///
    /// [`GpuError::HostRequirements`] names every feature, limit or format
    /// capability the context lacks.
    fn adopt(context: Self::Context, desc: &HostDeviceDesc) -> Result<Opened<Self>, GpuError>;

    /// Checks a host texture against what rendering needs and creates the
    /// view the renderer will attach.
    ///
    /// # Errors
    ///
    /// [`GpuError::HostTarget`] names every way the texture is unusable.
    fn import_target(
        &self,
        texture: &Self::ExternalTexture,
        render_format: TextureFormat,
    ) -> Result<(Self::TextureView, ImportedTarget), GpuError>;

    /// A shareable native handle to a texture this device created, so the host
    /// can sample or copy a renderer-owned target.
    fn export_texture(&self, texture: &Self::Texture) -> Self::ExternalTexture;
}
