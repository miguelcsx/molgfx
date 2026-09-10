//! Backend-neutral descriptor types.
//!
//! Descriptors that only describe (sizes, formats, usages, layouts) are
//! plain data. Descriptors that bind live resources carry borrows of the
//! device's associated types and are generic over it — a resource reference
//! cannot be plain data, and pretending otherwise is how backend types leak.

mod binding;
mod buffer;
mod pass;
mod pipeline;
mod ray_query;
mod sampler;
mod shader;
mod texture;

pub use binding::{
    AccelerationStructureBinding, AccelerationStructureLayoutEntry, BindGroupDesc, BindGroupEntry,
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, RayQueryBindGroupDesc,
    RayQueryBindGroupLayoutDesc, ShaderStages,
};
pub use buffer::{BufferDesc, BufferUsage};
pub use pass::{
    ColorAttachment, ComputePassDesc, DepthAttachment, DepthLoadOp, LoadOp, RenderPassDesc,
    TimestampWrites,
};
pub use pipeline::{
    BlendMode, ColorTarget, CompareFunction, ComputePipelineDesc, DepthState, PrimitiveTopology,
    RenderPipelineDesc,
};
pub use ray_query::{
    AabbGeometry, AabbGeometrySize, AccelerationGeometryFlags, AccelerationIndexFormat,
    AccelerationStructureFlags, AccelerationStructureUpdateMode, BlasBuildDesc, BlasDesc,
    BlasGeometries, BlasGeometrySizes, TlasDesc, TlasInstance, TriangleGeometry,
    TriangleGeometrySize,
};
pub use sampler::{FilterMode, SamplerDesc};
pub use shader::ShaderModuleDesc;
pub use texture::{
    TextureDesc, TextureDimension, TextureFormat, TextureUsage, TextureViewDesc, TextureWrite,
};
