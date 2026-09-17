//! Persistent screen-overlay records rebuilt only after overlay edits.

use super::buffers::{count, upload_grow, write_draw_args};
use super::label_types::glyph_bits;
use crate::error::RenderError;
use molgfx_core::{OverlayContent, Scene};
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, Device};

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct OverlayGpu {
    anchor: [f32; 4],
    geometry: [f32; 4],
    color_a: [f32; 4],
    color_b: [f32; 4],
    metadata: [u32; 4],
}

#[derive(Debug)]
pub(super) struct GpuOverlays<D: Device> {
    records: Option<D::Buffer>,
    args: Option<D::Buffer>,
    group: Option<D::BindGroup>,
    capacity: u64,
    synced: Option<u64>,
    count: u32,
    scratch: Vec<OverlayGpu>,
}

impl<D: Device> GpuOverlays<D> {
    pub(super) const fn new() -> Self {
        Self {
            records: None,
            args: None,
            group: None,
            capacity: 0,
            synced: None,
            count: 0,
            scratch: Vec::new(),
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let revision = scene.overlay_revision();
        if self.synced == Some(revision) {
            return Ok(false);
        }
        pack(scene, &mut self.scratch);
        let needed = byte_len::<OverlayGpu>(self.scratch.len());
        let rebind = self.records.is_none() || needed > self.capacity;
        upload_grow(
            device,
            queue,
            "screen overlay records",
            &self.scratch,
            &mut self.records,
            &mut self.capacity,
        )?;
        self.count = count(self.scratch.len());
        write_draw_args(
            device,
            queue,
            "screen overlay indirect arguments",
            6,
            self.count,
            &mut self.args,
        )?;
        if rebind {
            self.bind(device, layout);
        }
        self.synced = Some(revision);
        Ok(true)
    }

    fn bind(&mut self, device: &D, layout: &D::BindGroupLayout) {
        let Some(records) = &self.records else { return };
        self.group = Some(device.create_bind_group(&BindGroupDesc {
            label: "group2: screen overlays",
            layout,
            entries: &[BindGroupEntry::Buffer {
                binding: 0,
                buffer: records,
            }],
        }));
    }

    pub(super) fn draw(&self) -> Option<(&D::BindGroup, &D::Buffer)> {
        (self.count > 0).then_some((self.group.as_ref()?, self.args.as_ref()?))
    }
}

fn pack(scene: &Scene, records: &mut Vec<OverlayGpu>) {
    records.clear();
    let mut overlays: Vec<_> = scene
        .overlays()
        .filter(|(_, value)| value.visible())
        .collect();
    overlays.sort_by_key(|(handle, value)| (value.order(), handle.row()));
    for (_, overlay) in overlays {
        let anchor = overlay.anchor();
        match overlay.content() {
            OverlayContent::Text {
                text,
                color,
                size_pixels,
            } => {
                pack_text(records, anchor, text, *color, *size_pixels, [0.0, 0.0]);
            }
            OverlayContent::ColorLegend {
                title,
                range,
                colors,
                size_pixels,
            } => {
                records.push(record(
                    anchor,
                    [size_pixels[0], size_pixels[1], range[0], range[1]],
                    colors[0].to_f32(),
                    colors[1].to_f32(),
                    [1, 0, 0, 0],
                ));
                pack_text(
                    records,
                    anchor,
                    title,
                    molgfx_math::Rgba8::WHITE,
                    12.0,
                    [size_pixels[0] * 0.5, size_pixels[1] + 10.0],
                );
                pack_text(
                    records,
                    anchor,
                    &format!("{:.3}  {:.3}", range[0], range[1]),
                    molgfx_math::Rgba8::WHITE,
                    10.0,
                    [size_pixels[0] * 0.5, -9.0],
                );
            }
            OverlayContent::ScaleBar {
                length_angstrom,
                color,
                width_pixels,
            } => records.push(record(
                anchor,
                [*length_angstrom, *width_pixels, 0.0, 0.0],
                color.to_f32(),
                [0.0; 4],
                [2, 0, 0, 0],
            )),
            OverlayContent::CoordinateTripod {
                size_pixels,
                width_pixels,
            } => {
                for (axis, color) in [
                    molgfx_math::Rgba8::opaque(239, 68, 68),
                    molgfx_math::Rgba8::opaque(34, 197, 94),
                    molgfx_math::Rgba8::opaque(59, 130, 246),
                ]
                .into_iter()
                .enumerate()
                {
                    records.push(record(
                        anchor,
                        [*size_pixels, *width_pixels, 0.0, 0.0],
                        color.to_f32(),
                        [0.0; 4],
                        [3, u32::try_from(axis).map_or(0, |value| value), 0, 0],
                    ));
                }
            }
        }
    }
}

fn pack_text(
    records: &mut Vec<OverlayGpu>,
    anchor: molgfx_core::OverlayAnchor,
    text: &str,
    color: molgfx_math::Rgba8,
    size: f32,
    offset: [f32; 2],
) {
    let advance = size * 0.72;
    let count = u16::try_from(text.chars().count()).map_or(f32::from(u16::MAX), f32::from);
    for (index, glyph) in text.chars().enumerate() {
        let bits = glyph_bits(glyph);
        let index = u16::try_from(index).map_or(f32::from(u16::MAX), f32::from);
        let mut glyph_anchor = anchor;
        glyph_anchor.pixels[0] += offset[0] + (index + 0.5 - count * 0.5) * advance;
        glyph_anchor.pixels[1] += offset[1];
        records.push(record(
            glyph_anchor,
            [size * 0.625, size, 0.0, 0.0],
            color.to_f32(),
            [0.0; 4],
            [
                0,
                u32::try_from(bits & u64::from(u32::MAX)).map_or(u32::MAX, |value| value),
                u32::try_from(bits >> 32).map_or(u32::MAX, |value| value),
                0,
            ],
        ));
    }
}

fn record(
    anchor: molgfx_core::OverlayAnchor,
    geometry: [f32; 4],
    color_a: [f32; 4],
    color_b: [f32; 4],
    metadata: [u32; 4],
) -> OverlayGpu {
    OverlayGpu {
        anchor: [
            anchor.normalized[0],
            anchor.normalized[1],
            anchor.pixels[0],
            anchor.pixels[1],
        ],
        geometry,
        color_a,
        color_b,
        metadata,
    }
}

fn byte_len<T>(count: usize) -> u64 {
    u64::try_from(count.saturating_mul(std::mem::size_of::<T>())).map_or(u64::MAX, |value| value)
}
