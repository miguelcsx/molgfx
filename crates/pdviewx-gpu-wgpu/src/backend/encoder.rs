//! Command recording over wgpu.

use crate::convert;
use crate::device::WgpuDevice;
use pdviewx_gpu::{ColorAttachment, ComputePassDesc, DepthAttachment, RenderPassDesc};
use std::ops::Range;

/// One pipeline type covering both kinds; the abstraction sets whichever
/// matches the open pass.
#[derive(Debug)]
pub enum WgpuPipeline {
    /// A render pipeline.
    Render(wgpu::RenderPipeline),
    /// A compute pipeline.
    Compute(wgpu::ComputePipeline),
}

/// The wgpu command encoder.
#[derive(Debug)]
pub struct WgpuCommandEncoder {
    pub(crate) encoder: wgpu::CommandEncoder,
}

impl pdviewx_gpu::CommandEncoder<WgpuDevice> for WgpuCommandEncoder {
    type RenderPass<'e> = WgpuRenderPass<'e>;
    type ComputePass<'e> = WgpuComputePass<'e>;

    fn begin_render_pass<'e>(
        &'e mut self,
        desc: &RenderPassDesc<'_, WgpuDevice>,
    ) -> WgpuRenderPass<'e> {
        let colors: Vec<Option<wgpu::RenderPassColorAttachment<'_>>> = desc
            .colors
            .iter()
            .map(|ColorAttachment { view, load }| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: convert::color_load(*load),
                        store: wgpu::StoreOp::Store,
                    },
                })
            })
            .collect();
        let depth = desc.depth.as_ref().map(
            |DepthAttachment {
                 view,
                 load,
                 read_only,
             }| {
                wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: if *read_only {
                        None
                    } else {
                        Some(wgpu::Operations {
                            load: convert::depth_load(*load),
                            store: wgpu::StoreOp::Store,
                        })
                    },
                    stencil_ops: None,
                }
            },
        );
        let timestamp_writes = desc
            .timestamps
            .map(|timestamps| wgpu::RenderPassTimestampWrites {
                query_set: timestamps.queries,
                beginning_of_pass_write_index: timestamps.beginning,
                end_of_pass_write_index: timestamps.end,
            });
        WgpuRenderPass {
            pass: self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(desc.label),
                color_attachments: &colors,
                depth_stencil_attachment: depth,
                timestamp_writes,
                occlusion_query_set: None,
                multiview_mask: None,
            }),
        }
    }

    fn begin_compute_pass<'e>(
        &'e mut self,
        desc: &ComputePassDesc<'_, WgpuDevice>,
    ) -> WgpuComputePass<'e> {
        let timestamp_writes = desc
            .timestamps
            .map(|timestamps| wgpu::ComputePassTimestampWrites {
                query_set: timestamps.queries,
                beginning_of_pass_write_index: timestamps.beginning,
                end_of_pass_write_index: timestamps.end,
            });
        WgpuComputePass {
            pass: self
                .encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some(desc.label),
                    timestamp_writes,
                }),
        }
    }

    fn copy_buffer_to_buffer(
        &mut self,
        src: &wgpu::Buffer,
        src_offset: u64,
        dst: &wgpu::Buffer,
        dst_offset: u64,
        size: u64,
    ) {
        self.encoder
            .copy_buffer_to_buffer(src, src_offset, dst, dst_offset, size);
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &wgpu::Texture,
        origin: (u32, u32),
        size: (u32, u32),
        bytes_per_row: u32,
        dst: &wgpu::Buffer,
    ) {
        self.encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: src,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin.0,
                    y: origin.1,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: dst,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
    }

    fn resolve_query_set(
        &mut self,
        queries: &wgpu::QuerySet,
        range: Range<u32>,
        dst: &wgpu::Buffer,
        offset: u64,
    ) {
        self.encoder.resolve_query_set(queries, range, dst, offset);
    }
}

/// An open wgpu render pass.
#[derive(Debug)]
pub struct WgpuRenderPass<'e> {
    pass: wgpu::RenderPass<'e>,
}

impl pdviewx_gpu::RenderPassEncoder<WgpuDevice> for WgpuRenderPass<'_> {
    fn set_pipeline(&mut self, pipeline: &WgpuPipeline) {
        if let WgpuPipeline::Render(p) = pipeline {
            self.pass.set_pipeline(p);
        }
    }

    fn set_bind_group(&mut self, index: u32, group: &wgpu::BindGroup, dynamic_offsets: &[u32]) {
        self.pass.set_bind_group(index, group, dynamic_offsets);
    }

    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.pass.draw(vertices, instances);
    }

    fn draw_indirect(&mut self, args: &wgpu::Buffer, offset: u64) {
        self.pass.draw_indirect(args, offset);
    }
}

/// An open wgpu compute pass.
#[derive(Debug)]
pub struct WgpuComputePass<'e> {
    pass: wgpu::ComputePass<'e>,
}

impl pdviewx_gpu::ComputePassEncoder<WgpuDevice> for WgpuComputePass<'_> {
    fn set_pipeline(&mut self, pipeline: &WgpuPipeline) {
        if let WgpuPipeline::Compute(p) = pipeline {
            self.pass.set_pipeline(p);
        }
    }

    fn set_bind_group(&mut self, index: u32, group: &wgpu::BindGroup, dynamic_offsets: &[u32]) {
        self.pass.set_bind_group(index, group, dynamic_offsets);
    }

    fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        self.pass.dispatch_workgroups(x, y, z);
    }
}
