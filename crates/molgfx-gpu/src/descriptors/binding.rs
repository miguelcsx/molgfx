//! Bind-group layouts and bind groups.
//!
//! Bind groups are numbered by update frequency, a fixed convention every
//! layout declaration documents: group 0 per-frame (camera, lights, time),
//! group 1 per-pass, group 2 per-representation, group 3 per-material. This
//! ordering minimizes rebinds.

use crate::device::Device;

bitflags::bitflags! {
    /// Which stages see a binding.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct ShaderStages: u32 {
        /// Vertex stage.
        const VERTEX = 1;
        /// Fragment stage.
        const FRAGMENT = 1 << 1;
        /// Compute stage.
        const COMPUTE = 1 << 2;
    }
}

/// What kind of resource a binding slot holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindingType {
    /// A uniform buffer.
    Uniform,
    /// A storage buffer.
    Storage {
        /// Whether shaders only read it.
        read_only: bool,
    },
    /// A sampled 2-D texture.
    Texture {
        /// Whether the texel type is filterable float (versus unsigned
        /// integer, e.g. the entity-id channel).
        filterable: bool,
    },
    /// A sampled 3-D floating-point texture.
    Texture3dFloat {
        /// Whether hardware linear filtering is required.
        filterable: bool,
    },
    /// A sampled 3-D unsigned-integer texture.
    Texture3dUint,
    /// A write-only 3-D storage texture used by compute-generated fields.
    StorageTexture3dWrite {
        /// Portable storage format shared by the layout and texture.
        format: crate::TextureFormat,
    },
    /// A depth texture read texel by texel, never compared or filtered.
    ///
    /// Shaders declare it as `texture_2d<f32>` and read the depth from the
    /// first channel of a `textureLoad`.
    DepthTexture,
    /// A sampler.
    Sampler {
        /// Whether it is a comparison sampler.
        comparison: bool,
    },
}

/// One slot in a bind-group layout: index, visibility, kind. Every binding
/// index is declared here with a name; shaders never invent one.
#[derive(Clone, Copy, Debug)]
pub struct BindGroupLayoutEntry {
    /// Binding index within the group.
    pub binding: u32,
    /// Stages that access it.
    pub visibility: ShaderStages,
    /// The resource kind.
    pub ty: BindingType,
}

/// Everything needed to create a bind-group layout.
#[derive(Clone, Copy, Debug)]
pub struct BindGroupLayoutDesc<'a> {
    /// Debug label; by convention states the group's update frequency.
    pub label: &'static str,
    /// The slots, in binding order.
    pub entries: &'a [BindGroupLayoutEntry],
}

/// A live resource bound into a slot.
#[derive(Debug)]
pub enum BindGroupEntry<'a, D: Device> {
    /// A whole buffer.
    Buffer {
        /// Binding index.
        binding: u32,
        /// The buffer.
        buffer: &'a D::Buffer,
    },
    /// A contiguous slice of a buffer.
    ///
    /// Binding a range rather than a whole buffer is what lets one packed
    /// table be drawn as several groups: each group's shader sees its slice
    /// starting at instance zero, so no draw needs a first-instance offset —
    /// a capability the portable baseline does not guarantee.
    BufferRange {
        /// Binding index.
        binding: u32,
        /// The buffer holding every group.
        buffer: &'a D::Buffer,
        /// Byte offset of this group; a multiple of the device's storage
        /// binding alignment.
        offset: u64,
        /// Length of this group in bytes.
        size: u64,
    },
    /// A texture view.
    Texture {
        /// Binding index.
        binding: u32,
        /// The view.
        view: &'a D::TextureView,
    },
    /// A sampler.
    Sampler {
        /// Binding index.
        binding: u32,
        /// The sampler.
        sampler: &'a D::Sampler,
    },
}

/// Everything needed to create a bind group over a layout.
#[derive(Debug)]
pub struct BindGroupDesc<'a, D: Device> {
    /// Debug label.
    pub label: &'static str,
    /// The layout this group instantiates.
    pub layout: &'a D::BindGroupLayout,
    /// The bound resources.
    pub entries: &'a [BindGroupEntry<'a, D>],
}

/// One TLAS resource bound into a ray-query bind group.
#[derive(Clone, Copy, Debug)]
pub struct AccelerationStructureBinding<'a, D: Device> {
    /// Binding index within the group.
    pub binding: u32,
    /// Top-level acceleration structure.
    pub tlas: &'a D::Tlas,
}

/// One acceleration-structure slot in a ray-query bind-group layout.
#[derive(Clone, Copy, Debug)]
pub struct AccelerationStructureLayoutEntry {
    /// Binding index within the group.
    pub binding: u32,
    /// Shader stages that may issue ray queries.
    pub visibility: ShaderStages,
}

/// Layout containing regular slots and acceleration-structure slots.
#[derive(Debug)]
pub struct RayQueryBindGroupLayoutDesc<'a> {
    /// Diagnostic label.
    pub label: &'static str,
    /// Buffer, texture and sampler slots.
    pub entries: &'a [BindGroupLayoutEntry],
    /// TLAS slots.
    pub acceleration_structures: &'a [AccelerationStructureLayoutEntry],
}

/// Bind group containing regular resources and TLAS resources.
#[derive(Debug)]
pub struct RayQueryBindGroupDesc<'a, D: Device> {
    /// Diagnostic label.
    pub label: &'static str,
    /// Layout containing matching acceleration-structure entries.
    pub layout: &'a D::BindGroupLayout,
    /// Buffer, texture and sampler resources.
    pub entries: &'a [BindGroupEntry<'a, D>],
    /// TLAS resources.
    pub acceleration_structures: &'a [AccelerationStructureBinding<'a, D>],
}
