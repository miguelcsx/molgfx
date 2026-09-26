//! Device capability flags.
//!
//! The renderer branches on these flags, never on backend identity. Every
//! capability buys speed or a quality ceiling, never correctness: for each
//! flag there is a fallback path that renders the same content.

bitflags::bitflags! {
    /// The boolean capabilities, as one word.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct CapabilityFlags: u32 {
        /// Hardware acceleration structures and WGSL ray queries.
        const RAY_QUERY = 1;
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

/// Renderer-relevant usage support for one texture format.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TextureFormatCapabilities {
    /// The format may be bound as a sampled texture.
    pub sampled: bool,
    /// The format may be bound as a write-only storage texture.
    pub storage_write: bool,
}

/// What the opened device can do beyond the baseline.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Capabilities {
    /// The boolean capabilities.
    pub flags: CapabilityFlags,
    /// Largest single storage buffer binding, bytes.
    pub max_storage_buffer_bytes: u64,
    /// Maximum storage-buffer bindings visible to one shader stage.
    pub max_storage_buffers_per_shader_stage: u32,
    /// Largest 2-D texture dimension, texels.
    pub max_texture_dim: u32,
    /// Largest 3-D texture dimension, texels.
    pub max_texture_dim_3d: u32,
    /// `r32float` sampling and storage-write support.
    pub r32float: TextureFormatCapabilities,
    /// `rg32float` sampling and storage-write support.
    pub rg32float: TextureFormatCapabilities,
    /// `rgba32float` sampling and storage-write support.
    pub rgba32float: TextureFormatCapabilities,
}

/// Device ceilings relevant to acceleration-structure allocation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RayQueryLimits {
    /// Maximum primitives in one BLAS.
    pub max_blas_primitives: u32,
    /// Maximum geometry groups in one BLAS.
    pub max_blas_geometries: u32,
    /// Maximum instances in one TLAS.
    pub max_tlas_instances: u32,
    /// Maximum acceleration-structure bindings visible to one shader stage.
    pub max_bindings_per_shader_stage: u32,
}

impl Capabilities {
    /// Hardware acceleration-structure traversal is available.
    ///
    /// This is a resource capability, not evidence that a renderer has wired
    /// a complete hardware quality path.
    #[must_use]
    pub fn hardware_ray_tracing(&self) -> bool {
        self.ray_query()
    }

    /// Hardware acceleration structures and WGSL ray queries are available.
    #[must_use]
    pub fn ray_query(&self) -> bool {
        self.flags.contains(CapabilityFlags::RAY_QUERY)
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

#[cfg(test)]
#[path = "capabilities_tests.rs"]
mod tests;
