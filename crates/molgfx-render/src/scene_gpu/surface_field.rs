//! Building one shared surface field and the resources it owns.
//!
//! A field key resolves to these resources; the cache that holds them lives in
//! [`super::surface_cache`]. Splitting them keeps the identity question (what
//! makes two surfaces the same field) apart from the allocation question (what
//! one field owns and how its bind groups are described).

use super::record_cache::RecordGeometry;
use super::surface_components::{SurfaceComponentSync, SurfaceComponents};
use super::uniforms::RepresentationUniforms;
use crate::error::RenderError;
use molgfx_core::{Representation, SurfaceComponentPolicy, SurfaceComponentThreshold, SurfaceKind};
use molgfx_gpu::{
    BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue as _, TextureDesc,
    TextureDimension, TextureFormat, TextureUsage, TextureViewDesc,
};

/// The sampling policy a field is generated under.
///
/// These are the scalar inputs the generation and erosion dispatches read, in
/// the same order and bit form the key stores them, so a field and its key can
/// never disagree about what was sampled.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FieldSampling {
    pub(crate) probe: f32,
    pub(crate) sigma: f32,
    pub(crate) radius_scale: f32,
    pub(crate) isolevel: f32,
    pub(crate) grid_min: [f32; 3],
    pub(crate) grid_cell: f32,
    pub(crate) grid_size: [u32; 4],
}

/// Everything that forces a field to be generated, and nothing else.
///
/// The record key stands in for the sampled geometry: it carries the asset
/// identity, which fingerprints the coordinates, element, residue and radius
/// columns, so a coordinate or topology change is a different field while a
/// colour or opacity change is not.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct SurfaceFieldKey {
    pub(crate) geometry: RecordGeometry,
    /// Which physically distinct boundary this field is.
    pub(crate) kind: u32,
    /// Probe radius, Gaussian support, radius scale and iso-level, as bits.
    pub(crate) probe: u32,
    pub(crate) sigma: u32,
    pub(crate) radius_scale: u32,
    pub(crate) isolevel: u32,
    /// Local-space grid origin and spacing.
    pub(crate) grid_min: [u32; 3],
    pub(crate) grid_cell: u32,
    /// Sampled grid dimensions.
    pub(crate) grid_size: [u32; 4],
    /// Connected-component policy: threshold tag, payload and ceiling.
    pub(crate) components: [u64; 3],
}

impl SurfaceFieldKey {
    /// Builds the key for one representation's field.
    #[must_use]
    pub(crate) fn new(
        representation: &Representation,
        geometry: RecordGeometry,
        sampling: FieldSampling,
    ) -> Self {
        let FieldSampling {
            probe,
            sigma,
            radius_scale,
            isolevel,
            grid_min,
            grid_cell,
            grid_size,
        } = sampling;
        Self {
            geometry,
            kind: representation.params.surface_kind as u32,
            probe: probe.to_bits(),
            sigma: sigma.to_bits(),
            radius_scale: radius_scale.to_bits(),
            isolevel: isolevel.to_bits(),
            grid_min: grid_min.map(f32::to_bits),
            grid_cell: grid_cell.to_bits(),
            grid_size,
            components: component_key(representation.params.surface_components),
        }
    }

    /// Whether this boundary needs the reentrant rolling-probe erosion pass.
    ///
    /// Only the solvent-excluded surface has reentrant probe-contact patches;
    /// eroding any other field would report a boundary nobody asked for.
    #[must_use]
    pub(crate) const fn erodes(self) -> bool {
        self.kind == SurfaceKind::SolventExcluded as u32
    }

    /// Whether this field is generated from a Gaussian density rather than a
    /// union of atom spheres.
    #[must_use]
    pub(crate) const fn gaussian(self) -> bool {
        self.kind == SurfaceKind::Gaussian as u32
    }

    /// Whether the component filter runs over this field.
    #[must_use]
    pub(crate) const fn filters(self) -> bool {
        self.components[0] != 0
    }
}

/// The connected-component policy as comparable words.
fn component_key(policy: SurfaceComponentPolicy) -> [u64; 3] {
    let (tag, payload) = match policy.threshold() {
        SurfaceComponentThreshold::Disabled => (0, 0),
        SurfaceComponentThreshold::Area(area) => (1, area.to_bits()),
        SurfaceComponentThreshold::Volume(volume) => (2, volume.to_bits()),
        SurfaceComponentThreshold::Voxels(voxels) => (3, voxels),
    };
    [
        tag,
        payload,
        policy.maximum_components().map_or(0, u64::from),
    ]
}

/// One 3-D field texture and its bindable view.
#[derive(Debug)]
pub(super) struct FieldTexture<D: Device> {
    _texture: D::Texture,
    pub(super) view: D::TextureView,
}

impl<D: Device> FieldTexture<D> {
    pub(super) fn create(
        device: &D,
        dimensions: [u32; 3],
        label: &'static str,
        format: TextureFormat,
    ) -> Result<Self, RenderError> {
        let texture = device.create_texture(&TextureDesc {
            label,
            width: dimensions[0],
            height: dimensions[1],
            depth: dimensions[2],
            dimension: TextureDimension::D3,
            format,
            usage: TextureUsage::STORAGE_BINDING.union(TextureUsage::TEXTURE_BINDING),
        })?;
        let view = device.create_texture_view(&texture, &TextureViewDesc {});
        Ok(Self {
            _texture: texture,
            view,
        })
    }
}

/// Every GPU resource one field key owns, and the bind groups over them.
///
/// Held here rather than in the slot so that the field, its normals and its
/// component working set are generated once and read by every surface that
/// shares the key.
#[derive(Debug)]
pub(super) struct SharedField<D: Device> {
    /// Probe-inflated distance field: the generation input, and the shading
    /// field for boundaries that neither erode nor filter.
    pub(super) inflated: FieldTexture<D>,
    /// Eroded solvent-excluded field, for boundaries that erode.
    pub(super) field: Option<FieldTexture<D>>,
    /// Compact normals of whichever field shading reads.
    pub(super) normals: FieldTexture<D>,
    /// Connected-component working set, for policies that filter.
    pub(super) components: Option<SurfaceComponents<D>>,
    /// Generation output, erosion pair, normals pair and generation input.
    pub(super) output: D::BindGroup,
    pub(super) erosion: Option<D::BindGroup>,
    pub(super) normal_output: D::BindGroup,
    pub(super) input: D::BindGroup,
    /// The field's own copy of the representation uniforms.
    ///
    /// The field pass reads the boundary kind, probe, grid and radius scale out
    /// of this block. Those are exactly the values the key covers, so every
    /// surface sharing the key would write identical contents; owning the block
    /// keeps the field independent of any one slot's buffer.
    pub(super) _uniforms: D::Buffer,
    /// Rolling-probe offsets, sized and filled for this key's probe radius.
    pub(super) _erosion_offsets: Option<D::Buffer>,
    pub(super) erosion_offsets_size: u64,
    pub(super) dimensions: [u32; 3],
    /// Whether this field is generated from a Gaussian density.
    pub(super) gaussian: bool,
    /// Whether the field still owes its generation dispatch.
    ///
    /// Set once when the key is created and cleared by the record pass, so a
    /// field shared by four surfaces is generated once, not four times.
    pub(super) pending: bool,
}

impl<D: Device> SharedField<D> {
    /// The field shading reads: filtered, eroded or inflated, in that order.
    #[must_use]
    pub(super) fn shading_field(&self) -> &D::TextureView {
        if let Some(filtered) = self.components.as_ref().and_then(SurfaceComponents::field) {
            return filtered;
        }
        match &self.field {
            Some(field) => &field.view,
            None => &self.inflated.view,
        }
    }

    /// Device bytes this field holds.
    #[must_use]
    pub(super) fn resident_bytes(&self) -> u64 {
        let cells = self
            .dimensions
            .iter()
            .copied()
            .fold(1u64, |each, axis| each.saturating_mul(u64::from(axis)));
        // One R32Float field always, a second when the boundary erodes, one
        // Rgba8Snorm for the normals, and one more when the component filter
        // retains a working set.
        let fields = 1 + u64::from(self.field.is_some());
        let filtered = u64::from(self.components.is_some());
        cells
            .saturating_mul(4)
            .saturating_mul(fields + 1 + filtered)
            .saturating_add(self.erosion_offsets_size)
            .saturating_add(std::mem::size_of::<RepresentationUniforms>() as u64)
    }
}

/// Inputs one field needs from the scene.
pub(super) struct FieldSync<'a, D: Device> {
    pub(super) device: &'a D,
    pub(super) queue: &'a D::Queue,
    pub(super) output_layout: &'a D::BindGroupLayout,
    pub(super) input_layout: &'a D::BindGroupLayout,
    pub(super) erosion_layout: &'a D::BindGroupLayout,
    pub(super) normal_layout: &'a D::BindGroupLayout,
    pub(super) component_layout: &'a D::BindGroupLayout,
    pub(super) key: SurfaceFieldKey,
    /// The policy the key encodes, for the component uniforms.
    pub(super) policy: SurfaceComponentPolicy,
    pub(super) dimensions: [u32; 3],
    pub(super) atoms: &'a D::Buffer,
    /// The placement whose coordinate and BVH slices the field samples.
    pub(super) structure: &'a super::structure::GpuStructure<D>,
    pub(super) asset_arena: &'a super::asset_arena::AssetArena<D>,
    pub(super) compaction: &'a D::Buffer,
    pub(super) uniforms: &'a RepresentationUniforms,
}

/// Allocates one field's textures, bind groups and component working set.
pub(super) fn build_field<D: Device>(
    sync: &FieldSync<'_, D>,
) -> Result<SharedField<D>, RenderError> {
    let dimensions = sync.dimensions;
    let inflated = FieldTexture::create(
        sync.device,
        dimensions,
        "probe-inflated surface field",
        TextureFormat::R32Float,
    )?;
    let field = if sync.key.erodes() {
        Some(FieldTexture::create(
            sync.device,
            dimensions,
            "solvent-excluded surface field",
            TextureFormat::R32Float,
        )?)
    } else {
        None
    };
    let normals = FieldTexture::create(
        sync.device,
        dimensions,
        "continuous surface normals",
        TextureFormat::Rgba8Snorm,
    )?;
    let (erosion_offsets, erosion_offsets_size) = match &field {
        Some(_) => {
            let (buffer, size) = probe_buffer(sync)?;
            (Some(buffer), size)
        }
        None => (None, 0),
    };
    let uniforms = sync.device.create_buffer(&BufferDesc {
        label: "surface field parameters",
        size: std::mem::size_of::<RepresentationUniforms>() as u64,
        usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
    })?;
    sync.queue
        .write_buffer(&uniforms, 0, bytemuck::bytes_of(sync.uniforms));
    let components = if sync.key.filters() {
        let mut components = SurfaceComponents::new();
        components.sync(&SurfaceComponentSync {
            device: sync.device,
            queue: sync.queue,
            layout: sync.component_layout,
            source: &shading_field(field.as_ref(), &inflated).view,
            dimensions,
            cell: f32::from_bits(sync.key.grid_cell),
            isolevel: f32::from_bits(sync.key.isolevel),
            gaussian: sync.key.gaussian(),
            policy: sync.policy,
        })?;
        Some(components)
    } else {
        None
    };
    let groups = FieldGroups::create(
        sync,
        &inflated,
        field.as_ref(),
        &normals,
        &uniforms,
        erosion_offsets.as_ref(),
    );
    Ok(SharedField {
        inflated,
        field,
        normals,
        components,
        output: groups.output,
        erosion: groups.erosion,
        normal_output: groups.normal_output,
        input: groups.input,
        _uniforms: uniforms,
        _erosion_offsets: erosion_offsets,
        erosion_offsets_size,
        dimensions,
        gaussian: sync.key.gaussian(),
        pending: true,
    })
}

/// Every bind group one field needs, built together.
struct FieldGroups<D: Device> {
    output: D::BindGroup,
    erosion: Option<D::BindGroup>,
    normal_output: D::BindGroup,
    input: D::BindGroup,
}

impl<D: Device> FieldGroups<D> {
    /// Builds the groups over one field's textures and buffers.
    ///
    /// Kept apart from the allocation so a field's textures and the groups that
    /// read them are each described in one place, and so the generation input
    /// and the erosion pair can be read without the allocation around them.
    fn create(
        sync: &FieldSync<'_, D>,
        inflated: &FieldTexture<D>,
        field: Option<&FieldTexture<D>>,
        normals: &FieldTexture<D>,
        uniforms: &D::Buffer,
        erosion_offsets: Option<&D::Buffer>,
    ) -> Self {
        let shading = match field {
            Some(field) => field,
            None => inflated,
        };
        let output = sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field output",
            layout: sync.output_layout,
            entries: &[BindGroupEntry::Texture {
                binding: 0,
                view: &inflated.view,
            }],
        });
        let input = sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field input",
            layout: sync.input_layout,
            entries: &[
                BindGroupEntry::Buffer {
                    binding: 0,
                    buffer: sync.atoms,
                },
                sync.structure.coords_entry(sync.asset_arena, 1),
                sync.structure.bvh_nodes_entry(sync.asset_arena, 6),
                sync.structure.bvh_indices_entry(sync.asset_arena, 7),
                BindGroupEntry::Buffer {
                    binding: 8,
                    buffer: sync.compaction,
                },
                BindGroupEntry::Buffer {
                    binding: 9,
                    buffer: uniforms,
                },
            ],
        });
        let erosion = erosion_offsets.map(|offsets| {
            sync.device.create_bind_group(&BindGroupDesc {
                label: "surface field erosion",
                layout: sync.erosion_layout,
                entries: &[
                    BindGroupEntry::Texture {
                        binding: 0,
                        view: &inflated.view,
                    },
                    BindGroupEntry::Texture {
                        binding: 1,
                        view: &shading.view,
                    },
                    BindGroupEntry::Buffer {
                        binding: 2,
                        buffer: offsets,
                    },
                ],
            })
        });
        let normal_output = sync.device.create_bind_group(&BindGroupDesc {
            label: "surface field normals",
            layout: sync.normal_layout,
            entries: &[
                BindGroupEntry::Texture {
                    binding: 0,
                    view: &shading.view,
                },
                BindGroupEntry::Texture {
                    binding: 1,
                    view: &normals.view,
                },
            ],
        });
        Self {
            output,
            erosion,
            normal_output,
            input,
        }
    }
}

/// The field a shading pass reads, given an optional eroded field.
fn shading_field<'a, D: Device>(
    field: Option<&'a FieldTexture<D>>,
    inflated: &'a FieldTexture<D>,
) -> &'a FieldTexture<D> {
    match field {
        Some(field) => field,
        None => inflated,
    }
}

/// The rolling-probe offsets buffer for one field, filled on creation.
///
/// The offsets are the probe sample directions scaled by the probe radius, so
/// they belong to the key: a different probe is a different buffer rather than
/// an in-place rewrite a resident field would not see.
fn probe_buffer<D: Device>(sync: &FieldSync<'_, D>) -> Result<(D::Buffer, u64), RenderError> {
    let offsets = super::probe_offsets::probe_offsets(f32::from_bits(sync.key.probe));
    let bytes = bytemuck::cast_slice(&offsets);
    let size = u64::try_from(bytes.len())
        .map_err(|_| molgfx_gpu::GpuError::LimitExceeded {
            resource: "surface erosion probe table",
            limit: u64::from(u32::MAX),
        })?
        .next_power_of_two()
        .max(256);
    let buffer = sync.device.create_buffer(&BufferDesc {
        label: "surface erosion probe offsets",
        size,
        usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
    })?;
    sync.queue.write_buffer(&buffer, 0, bytes);
    Ok((buffer, size))
}
