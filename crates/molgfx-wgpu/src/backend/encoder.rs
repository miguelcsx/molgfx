//! Command recording over wgpu.

use super::resource::{WgpuBuffer, WgpuTexture};
use crate::convert;
use crate::device::WgpuDevice;
use molgfx_gpu::{
    BlasBuildDesc, BlasGeometries, ColorAttachment, ComputePassDesc, DepthAttachment, GpuError,
    RenderPassDesc,
};
use std::ops::Range;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "encoder_tests.rs"]
mod tests;

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

impl molgfx_gpu::CommandEncoder<WgpuDevice> for WgpuCommandEncoder {
    type RenderPass<'e> = WgpuRenderPass<'e>;
    type ComputePass<'e> = WgpuComputePass<'e>;

    fn begin_render_pass<'e>(
        &'e mut self,
        desc: &RenderPassDesc<'_, WgpuDevice>,
    ) -> WgpuRenderPass<'e> {
        // Every portable pass fits WebGPU's eight color attachments. Keep
        // descriptor conversion on the stack instead of allocating per pass.
        let inline: [_; 8] =
            std::array::from_fn(|index| desc.colors.get(index).map(color_attachment));
        let overflow;
        let colors = if let Some(colors) = inline.get(..desc.colors.len()) {
            colors
        } else {
            // Preserve the HAL descriptor for a future higher-limit device;
            // never truncate attachments or add a panic at this boundary.
            overflow = desc
                .colors
                .iter()
                .map(|color| Some(color_attachment(color)))
                .collect::<Vec<_>>();
            &overflow
        };
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
                color_attachments: colors,
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
        src: &WgpuBuffer,
        src_offset: u64,
        dst: &WgpuBuffer,
        dst_offset: u64,
        size: u64,
    ) {
        self.encoder
            .copy_buffer_to_buffer(&src.raw, src_offset, &dst.raw, dst_offset, size);
    }

    fn copy_texture_to_buffer(
        &mut self,
        src: &WgpuTexture,
        origin: (u32, u32),
        size: (u32, u32),
        bytes_per_row: u32,
        destination_offset: u64,
        dst: &WgpuBuffer,
    ) {
        self.encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &src.raw,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin.0,
                    y: origin.1,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &dst.raw,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: destination_offset,
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
        dst: &WgpuBuffer,
        offset: u64,
    ) {
        self.encoder
            .resolve_query_set(queries, range, &dst.raw, offset);
    }

    fn build_blas(&mut self, desc: &BlasBuildDesc<'_, WgpuDevice>) -> Result<(), GpuError> {
        match desc.geometries {
            BlasGeometries::Triangles(geometries) => {
                let sizes: Vec<_> = geometries
                    .iter()
                    .map(|geometry| wgpu::BlasTriangleGeometrySizeDescriptor {
                        vertex_format: wgpu::VertexFormat::Float32x3,
                        vertex_count: geometry.size.vertex_count,
                        index_format: geometry
                            .size
                            .indices
                            .map(|indices| super::ray_query::index_format(indices.0)),
                        index_count: geometry.size.indices.map(|indices| indices.1),
                        flags: super::ray_query::geometry_flags(geometry.size.flags),
                    })
                    .collect();
                let geometry = geometries
                    .iter()
                    .zip(&sizes)
                    .map(|(geometry, size)| wgpu::BlasTriangleGeometry {
                        size,
                        vertex_buffer: &geometry.vertex_buffer.raw,
                        first_vertex: geometry.first_vertex,
                        vertex_stride: geometry.vertex_stride,
                        index_buffer: geometry.indices.map(|indices| &indices.0.raw),
                        first_index: geometry.indices.map(|indices| indices.1),
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    })
                    .collect();
                let entry = wgpu::BlasBuildEntry {
                    blas: desc.blas,
                    geometry: wgpu::BlasGeometries::TriangleGeometries(geometry),
                };
                self.encoder.build_acceleration_structures([&entry], []);
            }
            BlasGeometries::Aabbs(geometries) => {
                let sizes: Vec<_> = geometries
                    .iter()
                    .map(|geometry| wgpu::BlasAABBGeometrySizeDescriptor {
                        primitive_count: geometry.size.primitive_count,
                        flags: super::ray_query::geometry_flags(geometry.size.flags),
                    })
                    .collect();
                let geometry = geometries
                    .iter()
                    .zip(&sizes)
                    .map(|(geometry, size)| wgpu::BlasAabbGeometry {
                        size,
                        stride: geometry.stride,
                        aabb_buffer: &geometry.buffer.raw,
                        primitive_offset: geometry.offset,
                    })
                    .collect();
                let entry = wgpu::BlasBuildEntry {
                    blas: desc.blas,
                    geometry: wgpu::BlasGeometries::AabbGeometries(geometry),
                };
                self.encoder.build_acceleration_structures([&entry], []);
            }
        }
        Ok(())
    }

    fn build_tlas(&mut self, tlas: &wgpu::Tlas) -> Result<(), GpuError> {
        self.encoder.build_acceleration_structures([], [tlas]);
        Ok(())
    }
}

fn color_attachment<'a>(
    ColorAttachment { view, load }: &ColorAttachment<'a, WgpuDevice>,
) -> wgpu::RenderPassColorAttachment<'a> {
    wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: convert::color_load(*load),
            store: wgpu::StoreOp::Store,
        },
    }
}

/// An open wgpu render pass.
#[derive(Debug)]
pub struct WgpuRenderPass<'e> {
    pass: wgpu::RenderPass<'e>,
}

impl molgfx_gpu::RenderPassEncoder<WgpuDevice> for WgpuRenderPass<'_> {
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

    fn draw_indirect(&mut self, args: &WgpuBuffer, offset: u64) {
        self.pass.draw_indirect(&args.raw, offset);
    }
}

/// An open wgpu compute pass.
#[derive(Debug)]
pub struct WgpuComputePass<'e> {
    pass: wgpu::ComputePass<'e>,
}

impl molgfx_gpu::ComputePassEncoder<WgpuDevice> for WgpuComputePass<'_> {
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
