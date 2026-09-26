//! Texture descriptors and formats.

/// The formats the engine renders with. A deliberate subset: every format
/// here is universally supported on the targeted backends, and backends
/// match on it exhaustively, so growing the set is a deliberate change.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextureFormat {
    /// 8-bit RGBA, linear.
    Rgba8Unorm,
    /// Signed normalized 8-bit RGBA; compact generated direction fields.
    Rgba8Snorm,
    /// 8-bit RGBA, sRGB-encoded.
    Rgba8UnormSrgb,
    /// 8-bit BGRA, linear (a common swapchain format).
    Bgra8Unorm,
    /// 8-bit BGRA, sRGB-encoded (a common swapchain format).
    Bgra8UnormSrgb,
    /// 16-bit float RGBA; the linear HDR working format.
    Rgba16Float,
    /// Two 16-bit float channels; compact screen-space motion vectors.
    Rg16Float,
    /// Two 32-bit float channels; conservative scalar-volume min/max pairs.
    Rg32Float,
    /// Four 32-bit float channels; portable storage fallback for volume bounds.
    Rgba32Float,
    /// Single 8-bit channel; ambient-occlusion and masks.
    R8Unorm,
    /// Single 32-bit unsigned integer; the entity-id channel.
    R32Uint,
    /// Single 32-bit float channel; scientific scalar density grids.
    R32Float,
    /// 32-bit float depth, used with reversed depth.
    Depth32Float,
}

/// Texture dimensionality supported by the portable renderer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TextureDimension {
    /// A conventional image or render target.
    #[default]
    D2,
    /// A volumetric scalar field.
    D3,
}

impl TextureFormat {
    /// Whether this is a depth format.
    #[must_use]
    pub fn is_depth(self) -> bool {
        matches!(self, Self::Depth32Float)
    }
}

bitflags::bitflags! {
    /// How a texture may be used.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct TextureUsage: u32 {
        /// Rendered to as a color or depth attachment.
        const RENDER_ATTACHMENT = 1;
        /// Sampled from shaders.
        const TEXTURE_BINDING = 1 << 1;
        /// Written from compute as a storage texture.
        const STORAGE_BINDING = 1 << 2;
        /// Source of copies.
        const COPY_SRC = 1 << 3;
        /// Destination of copies.
        const COPY_DST = 1 << 4;
    }
}

/// Everything needed to create a texture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextureDesc {
    /// Debug label.
    pub label: &'static str,
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// Depth in texels; one for 2-D textures.
    pub depth: u32,
    /// Whether the texture is 2-D or 3-D.
    pub dimension: TextureDimension,
    /// Texel format.
    pub format: TextureFormat,
    /// Permitted usages.
    pub usage: TextureUsage,
}

/// View parameters; the default views the whole texture.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TextureViewDesc {}

/// One tightly described host-to-texture upload.
#[derive(Clone, Copy, Debug)]
pub struct TextureWrite<'a> {
    /// Destination origin in texels.
    pub origin: [u32; 3],
    /// Written extent in texels.
    pub size: [u32; 3],
    /// Byte stride between adjacent rows.
    pub bytes_per_row: u32,
    /// Rows between adjacent depth slices.
    pub rows_per_image: u32,
    /// Borrowed source allocation.
    pub data: &'a [u8],
}
