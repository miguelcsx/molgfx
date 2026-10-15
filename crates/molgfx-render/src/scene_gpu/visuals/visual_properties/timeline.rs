//! Attribute-timeline dispatch resources.

use crate::error::RenderError;
use molgfx_core::{RowDomain, Scene, VisualAttributeRef};
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue};

use super::{AttributeTimelineConfig, AttributeTimelineGpu, VisualPropertyTable};

// Optional GPU materialization for temporal columns reused by multiple styles.

impl<D: Device> VisualPropertyTable<D> {
    pub(super) fn bind_timelines(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
    ) -> Result<(), RenderError> {
        self.timelines.clear();
        let Some(buffer) = self.buffer.get() else {
            return Ok(());
        };
        for column in self
            .columns
            .iter()
            .filter(|column| column.materialized_offset.is_some())
        {
            let Some(output) = column.materialized_offset else {
                continue;
            };
            let words = column.length.saturating_mul(column.stride_words);
            let config = device.create_buffer(&BufferDesc {
                label: "attribute timeline materialization configuration",
                size: std::mem::size_of::<AttributeTimelineConfig>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?;
            queue.write_buffer(
                &config,
                0,
                bytemuck::bytes_of(&AttributeTimelineConfig {
                    offsets: [
                        column.offset,
                        column.offset.saturating_add(words),
                        output,
                        column.offset.saturating_sub(1),
                    ],
                    counts: [words, 0, 0, 0],
                }),
            );
            let group = device.create_bind_group(&BindGroupDesc {
                label: "attribute timeline materialization",
                layout,
                entries: &[
                    BindGroupEntry::Buffer { binding: 0, buffer },
                    BindGroupEntry::Buffer {
                        binding: 1,
                        buffer: &config,
                    },
                ],
            });
            self.timelines.push(AttributeTimelineGpu {
                config,
                group,
                groups: crate::scene_gpu::dispatch::workgroups_2d(u64::from(words).div_ceil(64)),
                paged_plan: None,
            });
        }
        Ok(())
    }

    pub(in crate::scene_gpu) fn sync_paged_timelines(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        source: &D::Buffer,
        source_revision: u64,
        plans: &[crate::engine::chunk_draw_plan::ResidentAttributeMaterialization],
    ) -> Result<(), RenderError> {
        let same_layout = self.paged_source_revision == source_revision
            && self.paged_timelines.len() == plans.len()
            && self
                .paged_timelines
                .iter()
                .zip(plans)
                .all(|(timeline, plan)| {
                    timeline
                        .paged_plan
                        .is_some_and(|current| same_paged_layout(current, *plan))
                });
        if same_layout {
            for (timeline, plan) in self.paged_timelines.iter_mut().zip(plans) {
                if timeline.paged_plan != Some(*plan) {
                    queue.write_buffer(
                        &timeline.config,
                        0,
                        bytemuck::bytes_of(&paged_timeline_config(*plan)?),
                    );
                    timeline.paged_plan = Some(*plan);
                }
            }
            return Ok(());
        }
        self.paged_timelines.clear();
        for plan in plans {
            let config = device.create_buffer(&BufferDesc {
                label: "paged attribute timeline materialization configuration",
                size: std::mem::size_of::<AttributeTimelineConfig>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?;
            queue.write_buffer(
                &config,
                0,
                bytemuck::bytes_of(&paged_timeline_config(*plan)?),
            );
            let group = device.create_bind_group(&BindGroupDesc {
                label: "paged attribute timeline materialization",
                layout,
                entries: &[
                    BindGroupEntry::Buffer {
                        binding: 0,
                        buffer: source,
                    },
                    BindGroupEntry::Buffer {
                        binding: 1,
                        buffer: &config,
                    },
                ],
            });
            self.paged_timelines.push(AttributeTimelineGpu {
                config,
                group,
                groups: crate::scene_gpu::dispatch::workgroups_2d(
                    u64::from(plan.word_count).div_ceil(64),
                ),
                paged_plan: Some(*plan),
            });
        }
        self.paged_source_revision = source_revision;
        Ok(())
    }
}

pub(super) fn same_paged_layout(
    left: crate::engine::chunk_draw_plan::ResidentAttributeMaterialization,
    right: crate::engine::chunk_draw_plan::ResidentAttributeMaterialization,
) -> bool {
    left.ticket == right.ticket
        && left.start_byte_offset == right.start_byte_offset
        && left.end_byte_offset == right.end_byte_offset
        && left.output_byte_offset == right.output_byte_offset
        && left.word_count == right.word_count
}

pub(super) fn paged_timeline_config(
    plan: crate::engine::chunk_draw_plan::ResidentAttributeMaterialization,
) -> Result<AttributeTimelineConfig, RenderError> {
    let word_offset = |bytes: u64| {
        if !bytes.is_multiple_of(4) {
            return Err(molgfx_gpu::GpuError::LimitExceeded {
                resource: "paged attribute timeline offset",
                limit: u64::from(u32::MAX) * 4,
            }
            .into());
        }
        u32::try_from(bytes / 4).map_err(|_| {
            RenderError::from(molgfx_gpu::GpuError::LimitExceeded {
                resource: "paged attribute timeline offset",
                limit: u64::from(u32::MAX) * 4,
            })
        })
    };
    Ok(AttributeTimelineConfig {
        offsets: [
            word_offset(plan.start_byte_offset)?,
            word_offset(plan.end_byte_offset)?,
            word_offset(plan.output_byte_offset)?,
            0,
        ],
        counts: [plan.word_count, plan.interpolation.to_bits(), 1, 0],
    })
}

pub(super) fn reference_consumers(scene: &Scene, reference: VisualAttributeRef) -> u32 {
    let representations = scene
        .representations()
        .fold(0_usize, |count, (_, representation)| {
            count.saturating_add(representation.visual.as_ref().map_or(0, |style| {
                program_consumers(style.program(), reference, true)
            }))
        });
    let domains = scene
        .domain_visuals()
        .fold(0_usize, |count, (domain, descriptor)| {
            count.saturating_add(program_consumers(
                descriptor.style().program(),
                reference,
                !matches!(domain, RowDomain::Relations(_)),
            ))
        });
    let count = representations.saturating_add(domains);
    crate::fallback(u32::try_from(count), u32::MAX)
}

pub(super) fn program_consumers(
    program: &molgfx_core::VisualProgram,
    reference: VisualAttributeRef,
    split: bool,
) -> usize {
    if !program.attributes().contains(&reference) {
        return 0;
    }
    if !split {
        return 1;
    }
    let cull = [
        molgfx_core::VisualOutput::Visibility,
        molgfx_core::VisualOutput::RadiusScale,
        molgfx_core::VisualOutput::WidthScale,
        molgfx_core::VisualOutput::PositionOffset,
    ]
    .iter()
    .any(|output| program.output_register(*output).is_some());
    let shading = [
        molgfx_core::VisualOutput::BaseColor,
        molgfx_core::VisualOutput::Opacity,
        molgfx_core::VisualOutput::Emission,
        molgfx_core::VisualOutput::Roughness,
        molgfx_core::VisualOutput::Specular,
        molgfx_core::VisualOutput::MaterialStrength,
    ]
    .iter()
    .any(|output| program.output_register(*output).is_some());
    usize::from(cull)
        .saturating_add(usize::from(shading))
        .max(1)
}
