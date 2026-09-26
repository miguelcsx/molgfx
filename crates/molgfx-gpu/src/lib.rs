//! The backend-agnostic GPU hardware abstraction: device, queue, encoder and
//! surface traits.
//!
//! This is an interface crate: traits, descriptor structs, capability flags
//! and typed errors. No logic lives here. Backends implement the traits with
//! their own concrete resource types via associated types, so the renderer
//! above is generic over the device with no dynamic dispatch on the hot
//! path, and callers above the backend layer never name a backend.

#![forbid(unsafe_code)]

mod capabilities;
mod descriptors;
mod device;
mod encoder;
mod error;
mod queue;
mod residency;
mod surface;

pub use capabilities::{Capabilities, CapabilityFlags, RayQueryLimits, TextureFormatCapabilities};
pub use descriptors::{
    AabbGeometry, AabbGeometrySize, AccelerationGeometryFlags, AccelerationIndexFormat,
    AccelerationStructureBinding, AccelerationStructureFlags, AccelerationStructureLayoutEntry,
    AccelerationStructureUpdateMode, BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc,
    BindGroupLayoutEntry, BindingType, BlasBuildDesc, BlasDesc, BlasGeometries, BlasGeometrySizes,
    BlendMode, BufferDesc, BufferUsage, ColorAttachment, ColorTarget, CompareFunction,
    ComputePassDesc, ComputePipelineDesc, DepthAttachment, DepthLoadOp, DepthState, FilterMode,
    LoadOp, PrimitiveTopology, RayQueryBindGroupDesc, RayQueryBindGroupLayoutDesc, RenderPassDesc,
    RenderPipelineDesc, SamplerDesc, ShaderModuleDesc, ShaderStages, TextureDesc, TextureDimension,
    TextureFormat, TextureUsage, TextureViewDesc, TextureWrite, TimestampWrites, TlasDesc,
    TlasInstance, TriangleGeometry, TriangleGeometrySize,
};
pub use device::{
    Device, DeviceDesc, Opened, PowerPreference, RayQueryDevice, ResourceMemory, WindowSource,
    WindowTarget,
};
pub use encoder::{CommandEncoder, ComputePassEncoder, RayQueryCommandEncoder, RenderPassEncoder};
pub use error::GpuError;
pub use queue::Queue;
pub use residency::{
    ArenaAllocation, ArenaError, ArenaMetrics, CommandScratch, CommandScratchMetrics, FenceValue,
    PagedArena, Retirement, ScratchFull, UploadBackpressure, UploadMetrics, UploadReservation,
    UploadRing, UploadRingConfig, UploadState, UploadTicket,
};
pub use surface::{Surface, SurfaceConfig, SurfaceError, SurfaceFrame};
