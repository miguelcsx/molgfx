//! Device capability flags.
//!
//! The renderer branches on these flags, never on backend identity. Every
//! capability buys speed or a quality ceiling, never correctness: for each
//! flag there is a fallback path that renders the same content.

bitflags::bitflags! {
    /// The boolean capabilities, as one word.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct CapabilityFlags: u32 {
        /// Hardware ray tracing (acceleration-structure traversal).
        const HARDWARE_RAY_TRACING = 1;
        /// Mesh shading pipeline.
        const MESH_SHADERS = 1 << 1;
        /// Unbounded descriptor arrays.
        const BINDLESS = 1 << 2;
        /// GPU timestamp queries for profiling.
        const TIMESTAMP_QUERIES = 1 << 3;
        /// Subgroup (wave/warp) operations in compute.
        const SUBGROUP_OPS = 1 << 4;
    }
}

/// What the opened device can do beyond the baseline.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Capabilities {
    /// The boolean capabilities.
    pub flags: CapabilityFlags,
    /// Largest single storage buffer binding, bytes.
    pub max_storage_buffer_bytes: u64,
    /// Largest 2-D texture dimension, texels.
    pub max_texture_dim: u32,
    /// Largest 3-D texture dimension, texels.
    pub max_texture_dim_3d: u32,
}

impl Capabilities {
    /// Hardware ray tracing is available.
    #[must_use]
    pub fn hardware_ray_tracing(&self) -> bool {
        self.flags.contains(CapabilityFlags::HARDWARE_RAY_TRACING)
    }

    /// Mesh shading is available.
    #[must_use]
    pub fn mesh_shaders(&self) -> bool {
        self.flags.contains(CapabilityFlags::MESH_SHADERS)
    }

    /// Bindless descriptor arrays are available.
    #[must_use]
    pub fn bindless(&self) -> bool {
        self.flags.contains(CapabilityFlags::BINDLESS)
    }

    /// GPU timestamp queries are available.
    #[must_use]
    pub fn timestamp_queries(&self) -> bool {
        self.flags.contains(CapabilityFlags::TIMESTAMP_QUERIES)
    }

    /// Subgroup operations are available.
    #[must_use]
    pub fn subgroup_ops(&self) -> bool {
        self.flags.contains(CapabilityFlags::SUBGROUP_OPS)
    }
}
