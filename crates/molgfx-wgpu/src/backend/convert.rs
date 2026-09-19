//! Pure descriptor conversions: abstraction types to wgpu types.
//!
//! Every function here is a total mapping with no device access, so the
//! whole module is unit-testable without a GPU.

use molgfx_gpu::{
    BindingType, BlendMode, BufferUsage, CompareFunction, DepthLoadOp, FilterMode, LoadOp,
    PrimitiveTopology, ShaderStages, TextureDimension, TextureFormat, TextureUsage,
};

#[cfg(test)]
#[path = "convert_tests.rs"]
mod tests;

pub(crate) fn buffer_usage(usage: BufferUsage) -> wgpu::BufferUsages {
    let mut out = wgpu::BufferUsages::empty();
    let pairs = [
        (BufferUsage::VERTEX, wgpu::BufferUsages::VERTEX),
        (BufferUsage::INDEX, wgpu::BufferUsages::INDEX),
        (BufferUsage::UNIFORM, wgpu::BufferUsages::UNIFORM),
        (BufferUsage::STORAGE, wgpu::BufferUsages::STORAGE),
        (BufferUsage::INDIRECT, wgpu::BufferUsages::INDIRECT),
        (BufferUsage::COPY_SRC, wgpu::BufferUsages::COPY_SRC),
        (BufferUsage::COPY_DST, wgpu::BufferUsages::COPY_DST),
        (BufferUsage::MAP_READ, wgpu::BufferUsages::MAP_READ),
        (
            BufferUsage::QUERY_RESOLVE,
            wgpu::BufferUsages::QUERY_RESOLVE,
        ),
        (BufferUsage::BLAS_INPUT, wgpu::BufferUsages::BLAS_INPUT),
    ];
    for (ours, theirs) in pairs {
        if usage.contains(ours) {
            out |= theirs;
        }
    }
    out
}

pub(crate) fn texture_usage(usage: TextureUsage) -> wgpu::TextureUsages {
    let mut out = wgpu::TextureUsages::empty();
    let pairs = [
        (
            TextureUsage::RENDER_ATTACHMENT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        ),
        (
            TextureUsage::TEXTURE_BINDING,
            wgpu::TextureUsages::TEXTURE_BINDING,
        ),
        (
            TextureUsage::STORAGE_BINDING,
            wgpu::TextureUsages::STORAGE_BINDING,
        ),
        (TextureUsage::COPY_SRC, wgpu::TextureUsages::COPY_SRC),
        (TextureUsage::COPY_DST, wgpu::TextureUsages::COPY_DST),
    ];
    for (ours, theirs) in pairs {
        if usage.contains(ours) {
            out |= theirs;
        }
    }
    out
}

pub(crate) fn texture_format(format: TextureFormat) -> wgpu::TextureFormat {
    match format {
        TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8Snorm => wgpu::TextureFormat::Rgba8Snorm,
        TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        TextureFormat::Rg16Float => wgpu::TextureFormat::Rg16Float,
        TextureFormat::Rg32Float => wgpu::TextureFormat::Rg32Float,
        TextureFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
        TextureFormat::R32Uint => wgpu::TextureFormat::R32Uint,
        TextureFormat::R32Float => wgpu::TextureFormat::R32Float,
        TextureFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
    }
}

/// The inverse mapping for the swapchain formats a surface may choose.
pub(crate) fn surface_format_back(format: wgpu::TextureFormat) -> Option<TextureFormat> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm => Some(TextureFormat::Rgba8Unorm),
        wgpu::TextureFormat::Rgba8UnormSrgb => Some(TextureFormat::Rgba8UnormSrgb),
        wgpu::TextureFormat::Bgra8Unorm => Some(TextureFormat::Bgra8Unorm),
        wgpu::TextureFormat::Bgra8UnormSrgb => Some(TextureFormat::Bgra8UnormSrgb),
        _ => None,
    }
}

pub(crate) fn shader_stages(stages: ShaderStages) -> wgpu::ShaderStages {
    let mut out = wgpu::ShaderStages::empty();
    if stages.contains(ShaderStages::VERTEX) {
        out |= wgpu::ShaderStages::VERTEX;
    }
    if stages.contains(ShaderStages::FRAGMENT) {
        out |= wgpu::ShaderStages::FRAGMENT;
    }
    if stages.contains(ShaderStages::COMPUTE) {
        out |= wgpu::ShaderStages::COMPUTE;
    }
    out
}

pub(crate) fn binding_type(ty: BindingType) -> wgpu::BindingType {
    match ty {
        BindingType::Uniform => wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        BindingType::Storage { read_only } => wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        BindingType::Texture { filterable } => wgpu::BindingType::Texture {
            sample_type: if filterable {
                wgpu::TextureSampleType::Float { filterable: true }
            } else {
                wgpu::TextureSampleType::Uint
            },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        BindingType::Texture3dFloat { filterable } => wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D3,
            multisampled: false,
        },
        BindingType::Texture3dUint => wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Uint,
            view_dimension: wgpu::TextureViewDimension::D3,
            multisampled: false,
        },
        BindingType::StorageTexture3dWrite { format } => wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: texture_format(format),
            view_dimension: wgpu::TextureViewDimension::D3,
        },
        BindingType::DepthTexture => wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        BindingType::Sampler { comparison } => wgpu::BindingType::Sampler(if comparison {
            wgpu::SamplerBindingType::Comparison
        } else {
            wgpu::SamplerBindingType::Filtering
        }),
    }
}

pub(crate) fn texture_dimension(dimension: TextureDimension) -> wgpu::TextureDimension {
    match dimension {
        TextureDimension::D2 => wgpu::TextureDimension::D2,
        TextureDimension::D3 => wgpu::TextureDimension::D3,
    }
}

pub(crate) fn compare_function(f: CompareFunction) -> wgpu::CompareFunction {
    match f {
        CompareFunction::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
        CompareFunction::Greater => wgpu::CompareFunction::Greater,
        CompareFunction::LessEqual => wgpu::CompareFunction::LessEqual,
        CompareFunction::Always => wgpu::CompareFunction::Always,
    }
}

pub(crate) fn blend_state(mode: BlendMode) -> Option<wgpu::BlendState> {
    match mode {
        BlendMode::Replace => None,
        BlendMode::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
        BlendMode::Additive => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        BlendMode::ReverseMultiply => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::OneMinusSrc,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        }),
    }
}

pub(crate) fn topology(t: PrimitiveTopology) -> wgpu::PrimitiveTopology {
    match t {
        PrimitiveTopology::TriangleList => wgpu::PrimitiveTopology::TriangleList,
    }
}

pub(crate) fn filter_mode(f: FilterMode) -> wgpu::FilterMode {
    match f {
        FilterMode::Nearest => wgpu::FilterMode::Nearest,
        FilterMode::Linear => wgpu::FilterMode::Linear,
    }
}

pub(crate) fn color_load(load: LoadOp) -> wgpu::LoadOp<wgpu::Color> {
    match load {
        LoadOp::Clear([r, g, b, a]) => wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
        LoadOp::Load => wgpu::LoadOp::Load,
    }
}

pub(crate) fn depth_load(load: DepthLoadOp) -> wgpu::LoadOp<f32> {
    match load {
        DepthLoadOp::Clear(depth) => wgpu::LoadOp::Clear(depth),
        DepthLoadOp::Load => wgpu::LoadOp::Load,
    }
}
