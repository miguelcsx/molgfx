//! Buffer descriptors.

bitflags::bitflags! {
    /// How a buffer may be used. Buffers are typed by usage and allocated
    /// once; there is deliberately no per-frame creation convenience.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct BufferUsage: u32 {
        /// Bound as a vertex buffer.
        const VERTEX = 1;
        /// Bound as an index buffer.
        const INDEX = 1 << 1;
        /// Bound as a uniform buffer.
        const UNIFORM = 1 << 2;
        /// Bound as a storage buffer.
        const STORAGE = 1 << 3;
        /// Source of indirect draw or dispatch arguments.
        const INDIRECT = 1 << 4;
        /// Source of GPU-to-GPU copies.
        const COPY_SRC = 1 << 5;
        /// Destination of uploads and GPU-to-GPU copies.
        const COPY_DST = 1 << 6;
        /// Mappable for reading back to the host. Off the frame path only:
        /// golden-image capture, picking, export.
        const MAP_READ = 1 << 7;
        /// Destination for resolved GPU timestamp or occlusion queries.
        const QUERY_RESOLVE = 1 << 8;
    }
}

/// Everything needed to create a buffer.
#[derive(Clone, Copy, Debug)]
pub struct BufferDesc {
    /// Debug label, surfaced in captures and validation messages.
    pub label: &'static str,
    /// Size in bytes.
    pub size: u64,
    /// Permitted usages.
    pub usage: BufferUsage,
}
