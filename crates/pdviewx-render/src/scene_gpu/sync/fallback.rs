//! Small fallback textures used by optional scene GPU bindings.

use crate::error::RenderError;
use pdviewx_gpu::{
    Device, TextureDesc, TextureDimension, TextureFormat, TextureUsage, TextureViewDesc,
};

pub(super) fn fallback_texture<D: Device>(
    device: &D,
    label: &'static str,
    format: TextureFormat,
) -> Result<(D::Texture, D::TextureView), RenderError> {
    let texture = device.create_texture(&TextureDesc {
        label,
        width: 2,
        height: 2,
        depth: 2,
        dimension: TextureDimension::D3,
        format,
        usage: TextureUsage::TEXTURE_BINDING,
    })?;
    let view = device.create_texture_view(&texture, &TextureViewDesc::default());
    Ok((texture, view))
}
