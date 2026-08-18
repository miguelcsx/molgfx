//! Command recording traits.
//!
//! Pass encoders borrow the command encoder for their lifetime (generic
//! associated types), matching how every modern API scopes a pass.

use crate::descriptors::{ComputePassDesc, RenderPassDesc};
use crate::device::Device;
use std::ops::Range;

/// Records passes and copies into one submission.
pub trait CommandEncoder<D: Device>: Sized {
    /// The recording handle of an open render pass.
    type RenderPass<'e>: RenderPassEncoder<D>
    where
        Self: 'e;
    /// The recording handle of an open compute pass.
    type ComputePass<'e>: ComputePassEncoder<D>
    where
        Self: 'e;

    /// Begins a render pass over the given attachments.
    fn begin_render_pass<'e>(&'e mut self, desc: &RenderPassDesc<'_, D>) -> Self::RenderPass<'e>;

    /// Begins a compute pass.
    fn begin_compute_pass<'e>(&'e mut self, desc: &ComputePassDesc<'_, D>)
    -> Self::ComputePass<'e>;

    /// GPU-to-GPU buffer copy; how indirect-argument templates reset
    /// per-frame counts without a host write.
    fn copy_buffer_to_buffer(
        &mut self,
        src: &D::Buffer,
        src_offset: u64,
        dst: &D::Buffer,
        dst_offset: u64,
        size: u64,
    );

    /// Copies one texture's contents into a buffer, tightly packed rows.
    /// Off the frame path except for picking's few-texel read.
    fn copy_texture_to_buffer(
        &mut self,
        src: &D::Texture,
        origin: (u32, u32),
        size: (u32, u32),
        bytes_per_row: u32,
        dst: &D::Buffer,
    );

    /// Resolves query values into a GPU buffer for later copy/readback.
    fn resolve_query_set(
        &mut self,
        queries: &D::QuerySet,
        range: Range<u32>,
        dst: &D::Buffer,
        offset: u64,
    );
}

/// Records draws inside an open render pass.
pub trait RenderPassEncoder<D: Device> {
    /// Sets the active pipeline.
    fn set_pipeline(&mut self, pipeline: &D::Pipeline);

    /// Binds a group at an index (0 frame / 1 pass / 2 representation /
    /// 3 material), with dynamic offsets where the layout declared them.
    fn set_bind_group(&mut self, index: u32, group: &D::BindGroup, dynamic_offsets: &[u32]);

    /// Direct draw; used only for fullscreen passes whose vertex count is a
    /// constant, never per scene primitive.
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>);

    /// Indirect draw against arguments a compute pass wrote; the only draw
    /// the scene-geometry path uses.
    fn draw_indirect(&mut self, args: &D::Buffer, offset: u64);
}

/// Records dispatches inside an open compute pass.
pub trait ComputePassEncoder<D: Device> {
    /// Sets the active pipeline.
    fn set_pipeline(&mut self, pipeline: &D::Pipeline);

    /// Binds a group at an index.
    fn set_bind_group(&mut self, index: u32, group: &D::BindGroup, dynamic_offsets: &[u32]);

    /// Dispatches workgroups.
    fn dispatch(&mut self, x: u32, y: u32, z: u32);
}
