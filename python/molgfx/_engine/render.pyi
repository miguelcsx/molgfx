"""The renderer.

The engine, its configuration, the render profiles, and the chunk and brick
residency that feeds it.
"""

from typing import Sequence, final

import numpy as np
from numpy.typing import NDArray

from .core import AnalyticTemplate, MemoryOwnership, MemoryTransferExclusion, RelationStyle, Scene, SelectionHandle, TrajectoryChunkWindow, VolumeSegmentRef
from .math import Camera, Mat4, Rgba8, Vec3
from .semantic import ChunkId, DatasetCatalog, DatasetId, ResidencyRequest, ResidencyTicket

__all__ = [
    "BrickAddress",
    "BrickCatalog",
    "BrickDescriptor",
    "BrickId",
    "BrickMetadata",
    "BrickShape",
    "BrickValueRange",
    "DirtyGeneration",
    "BrickAtlasConfig",
    "BrickAtlasKind",
    "BrickAtlasMetrics",
    "FenceValue",
    "UploadRingConfig",
    "ChunkPlacementId",
    "ChunkPlacementStatus",
    "ChunkRepresentation",
    "StructureChunkPlacement",
    "BondChunkPlacement",
    "RelationChunkPlacement",
    "InstanceChunkWindow",
    "AttributeChunkWindow",
    "InstanceChunkPlacement",
    "PointChunkPlacement",
    "ResidentGenericChunk",
    "ChunkResidencyMetrics",
    "ArenaMetrics",
    "UploadMetrics",
    "ResidentStructureChunk",
    "ResidentTrajectoryChunk",
    "DerivedCacheUsage",
    "FrameCompleteness",
    "FrameDegradation",
    "FrameMetrics",
    "Capabilities",
    "DerivedCacheBudget",
    "EngineConfig",
    "FrameReport",
    "FrameStatus",
    "FrameTicket",
    "ImageConfig",
    "PowerPreference",
    "RenderMode",
    "SequenceConfig",
    "Engine",
    "FrameTiming",
    "GlobalPickIdentity",
    "HdrImage",
    "Image",
    "Pick",
    "PickEntity",
    "SequenceFrame",
    "SequenceRenderer",
    "RenderProfile",
    "ResolvedRenderPlan",
    "BackdropStyle",
    "IllustrationStyle",
    "LightingEnvironment",
    "DisplayGamut",
    "DisplayTransform",
    "ToneMapping",
    "TransferFunction",
    "BloomStyle",
    "DepthOfField",
    "EffectLayer",
    "MotionBlur",
    "PresentationEffect",
    "FocusTarget",
    "RenderSession",
]

@final
class BrickId:
    def __new__(cls, value: int) -> BrickId: ...
    value: int
    def __eq__(self, other: object, /) -> bool: ...

@final
class BrickAddress:
    def __new__(cls, origin: Sequence[int], mip: int) -> BrickAddress: ...
    origin: list[int]
    mip: int

@final
class DirtyGeneration:
    def __new__(cls, value: int) -> DirtyGeneration: ...
    value: int
    def next(self) -> DirtyGeneration: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class BrickValueRange:
    @staticmethod
    def scalar(min: float, max: float) -> BrickValueRange: ...
    @staticmethod
    def segmentation(min: int, max: int) -> BrickValueRange: ...
    @staticmethod
    def occupancy(has_empty: bool, has_occupied: bool) -> BrickValueRange: ...
    kind: str

@final
class BrickShape:
    def __new__(cls, stored: Sequence[int], halo: int) -> BrickShape: ...
    stored: list[int]
    interior: list[int]
    halo: int
    voxel_count: int

@final
class BrickMetadata:
    def __new__(cls, id: BrickId, address: BrickAddress, shape: BrickShape, value_range: BrickValueRange, generation: DirtyGeneration) -> BrickMetadata: ...
    id: BrickId
    address: BrickAddress
    shape: BrickShape
    value_range: BrickValueRange
    generation: DirtyGeneration

@final
class BrickDescriptor:
    def __new__(cls, chunk: ChunkId, metadata: BrickMetadata) -> BrickDescriptor: ...
    chunk: ChunkId
    metadata: BrickMetadata

@final
class BrickCatalog:
    @staticmethod
    def from_descriptors(dataset_id: DatasetId, logical_extent: Sequence[int], voxel_bytes: int, descriptors: Sequence[BrickDescriptor]) -> BrickCatalog: ...
    dataset_id: DatasetId
    logical_extent: list[int]
    descriptor_count: int
    def copy_descriptors(self) -> list[BrickDescriptor]: ...
    def logical_bytes(self) -> int: ...

@final
class UploadRingConfig:
    def __new__(cls, capacity_bytes: int, ticket_capacity: int, epoch_budget_bytes: int, in_flight_budget_bytes: int, alignment: int) -> UploadRingConfig: ...
    capacity_bytes: int
    ticket_capacity: int
    epoch_budget_bytes: int
    in_flight_budget_bytes: int
    alignment: int

@final
class BrickAtlasKind:
    Scalar: BrickAtlasKind
    Segmentation: BrickAtlasKind
    Occupancy: BrickAtlasKind
    Surface: BrickAtlasKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class BrickAtlasConfig:
    def __new__(cls, stored_shape: Sequence[int], resident_capacity: int, uploads: UploadRingConfig, kind: BrickAtlasKind) -> BrickAtlasConfig: ...
    stored_shape: list[int]
    resident_capacity: int
    uploads: UploadRingConfig
    kind: BrickAtlasKind

@final
class FenceValue:
    value: int
    def __eq__(self, other: object, /) -> bool: ...

@final
class BrickAtlasMetrics:
    atlas_bytes: int
    page_table_bytes: int
    resident_pages: int
    pending_uploads: int
    pending_evictions: int
    stale_completions: int
    page_table_writes: int

@final
class PointChunkPlacement:
    def __new__(cls, id: ChunkPlacementId, ticket: ResidencyTicket, model_to_world: Mat4, diameter_pixels: float, color: Rgba8) -> PointChunkPlacement: ...
    id: ChunkPlacementId
    ticket: ResidencyTicket
    model_to_world: Mat4
    diameter_pixels: float
    color: Rgba8

@final
class InstanceChunkPlacement:
    def __new__(cls, id: ChunkPlacementId, ticket: ResidencyTicket, template: AnalyticTemplate, color: Rgba8) -> InstanceChunkPlacement: ...
    id: ChunkPlacementId
    ticket: ResidencyTicket
    color: Rgba8

@final
class ChunkPlacementId:
    def __new__(cls, value: int) -> ChunkPlacementId: ...
    value: int
    def __eq__(self, other: object, /) -> bool: ...

@final
class ChunkRepresentation:
    @staticmethod
    def points(diameter_pixels: float, color: Rgba8) -> ChunkRepresentation: ...
    @staticmethod
    def spacefill(radius_scale: float, color: Rgba8) -> ChunkRepresentation: ...
    kind: str
    diameter_pixels: float | None
    radius_scale: float | None
    color: Rgba8
    def __repr__(self) -> str: ...

@final
class StructureChunkPlacement:
    def __new__(cls, id: ChunkPlacementId, ticket: ResidencyTicket, model_to_world: Mat4, representation: ChunkRepresentation) -> StructureChunkPlacement: ...
    id: ChunkPlacementId
    ticket: ResidencyTicket
    model_to_world: Mat4
    representation: ChunkRepresentation

@final
class BondChunkPlacement:
    @staticmethod
    def licorice(id: ChunkPlacementId, ticket: ResidencyTicket, model_to_world: Mat4, radius: float, color: Rgba8) -> BondChunkPlacement: ...
    id: ChunkPlacementId
    ticket: ResidencyTicket
    model_to_world: Mat4
    radius: float
    color: Rgba8

@final
class RelationChunkPlacement:
    def __new__(cls, id: ChunkPlacementId, ticket: ResidencyTicket, style: RelationStyle) -> RelationChunkPlacement: ...
    id: ChunkPlacementId
    ticket: ResidencyTicket
    style: RelationStyle

@final
class InstanceChunkWindow:
    def __new__(cls, placement: ChunkPlacementId, start: ResidencyTicket, end: ResidencyTicket, interpolation: float) -> InstanceChunkWindow: ...
    placement: ChunkPlacementId
    start: ResidencyTicket
    end: ResidencyTicket
    interpolation: float

@final
class AttributeChunkWindow:
    def __new__(cls, attribute: ResidencyTicket, start: ResidencyTicket, end: ResidencyTicket, interpolation: float) -> AttributeChunkWindow: ...
    attribute: ResidencyTicket
    start: ResidencyTicket
    end: ResidencyTicket
    interpolation: float

@final
class ChunkResidencyMetrics:
    arena: ArenaMetrics
    uploads: UploadMetrics
    tracked_chunks: int
    tracked_capacity: int

@final
class ArenaMetrics:
    resident_bytes: int
    peak_resident_bytes: int
    requested_bytes: int
    allocated_bytes: int
    allocations: int
    allocation_stalls: int
    stalled_bytes: int
    host_allocation_events: int

@final
class UploadMetrics:
    bytes_staged: int
    bytes_submitted: int
    bytes_retired: int
    bytes_cancelled: int
    epoch_bytes: int
    in_flight_bytes: int
    peak_in_flight_bytes: int
    occupied_bytes: int
    peak_occupied_bytes: int
    active_tickets: int
    stall_events: int
    stalled_bytes: int
    host_allocation_events: int

@final
class ResidentStructureChunk:
    ticket: ResidencyTicket
    byte_offset: int
    byte_len: int
    local_rows: int
    cluster_offset: int
    cluster_count: int

@final
class ResidentTrajectoryChunk:
    ticket: ResidencyTicket
    byte_offset: int
    byte_len: int
    local_rows: int

@final
class DerivedCacheUsage:
    cpu_bytes: int
    gpu_bytes: int
    peak_cpu_bytes: int
    peak_gpu_bytes: int

@final
class FrameCompleteness:
    Complete: FrameCompleteness
    @staticmethod
    def progressive(pending_chunks: int) -> FrameCompleteness: ...
    is_complete: bool
    pending_chunks: int
    def __repr__(self) -> str: ...

@final
class FrameDegradation:
    STREAMING_PROXY: FrameDegradation
    def contains(self, feature: FrameDegradation) -> bool: ...
    def __repr__(self) -> str: ...

@final
class FrameMetrics:
    tracked_chunks: int
    upload_in_flight_bytes: int
    derived_cache_gpu_bytes: int
    derived_cache_peak_gpu_bytes: int
    physical_buffer_bytes: int
    physical_texture_bytes: int
    physical_total_bytes: int
    physical_peak_bytes: int

@final
class ChunkPlacementStatus:
    Missing: ChunkPlacementStatus
    NotResident: ChunkPlacementStatus
    Resident: ChunkPlacementStatus
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ResidentGenericChunk:
    ticket: ResidencyTicket
    byte_offset: int
    byte_len: int
    local_rows: int
    stride: int

@final
class PowerPreference:
    HighPerformance: PowerPreference
    LowPower: PowerPreference
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class RenderMode:
    Cinematic: RenderMode
    Realtime: RenderMode
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class FrameStatus:
    Presented: FrameStatus
    Skipped: FrameStatus
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class FrameReport:
    status: FrameStatus
    complete: bool
    pending_chunks: int
    streaming_proxy: bool
    needs_another_frame: bool
    tracked_chunks: int
    upload_in_flight_bytes: int
    derived_cache_gpu_bytes: int
    derived_cache_peak_gpu_bytes: int
    completeness: FrameCompleteness
    degradation: FrameDegradation
    metrics: FrameMetrics

@final
class DerivedCacheBudget:
    def __new__(cls, cpu_bytes: int, gpu_bytes: int) -> DerivedCacheBudget: ...
    cpu_bytes: int
    gpu_bytes: int
    @staticmethod
    def default() -> DerivedCacheBudget: ...

@final
class ImageConfig:
    def __new__(cls, width: int, height: int) -> ImageConfig: ...
    width: int
    height: int
    @staticmethod
    def publication_4k() -> ImageConfig: ...

@final
class SequenceConfig:
    def __new__(cls, width: int, height: int, frames_per_second: int, max_in_flight: int = 3) -> SequenceConfig: ...
    width: int
    height: int
    timebase_nanoseconds: int
    max_in_flight: int

@final
class FrameTicket:
    index: int
    timestamp: int

@final
class EngineConfig:
    def __new__(cls, width: int = 1280, height: int = 800, mode: RenderMode | None = None, profile: RenderProfile | None = None, power: PowerPreference | None = None, derived_cache: DerivedCacheBudget | None = None) -> EngineConfig: ...
    width: int
    height: int
    mode: RenderMode
    profile: RenderProfile
    power: PowerPreference
    derived_cache: DerivedCacheBudget
    @staticmethod
    def default() -> EngineConfig: ...

@final
class Capabilities:
    max_storage_buffer_bytes: int
    max_texture_dim: int
    max_texture_dim_3d: int
    hardware_ray_tracing: bool
    mesh_shaders: bool
    bindless: bool
    timestamp_queries: bool
    subgroup_ops: bool

@final
class Image:
    width: int
    height: int
    numpy_ownership: MemoryOwnership
    buffer_pointer: int | None
    transfer_deleter: str
    @staticmethod
    def copy_from_numpy(pixels: NDArray[np.uint8]) -> Image: ...
    def transfer_numpy(self) -> NDArray[np.uint8]: ...
    def copy_png_bytes(self) -> bytes: ...

@final
class HdrImage:
    width: int
    height: int
    numpy_ownership: MemoryOwnership
    transfer_exclusion: MemoryTransferExclusion
    def copy_rgba16f_numpy(self) -> NDArray[np.uint8]: ...
    def write_exr(self, path: str) -> None: ...

@final
class FrameTiming:
    gpu_ns: int
    cpu_ns: int
    frame_ns: int

@final
class PickEntity:
    kind: str
    global_identity: GlobalPickIdentity | None
    volume_segment: VolumeSegmentRef | None

@final
class GlobalPickIdentity:
    dataset: int
    chunk: int
    row: int
    kind: str

@final
class Pick:
    entity: PickEntity
    selection_indices: NDArray[np.uint32]

@final
class Engine:
    def __new__(cls, config: EngineConfig | None = None) -> Engine: ...
    render_mode: RenderMode
    capabilities: Capabilities
    render_profile: RenderProfile
    resolved_render_plan: ResolvedRenderPlan
    def resize(self, width: int, height: int) -> None: ...
    def set_render_mode(self, mode: RenderMode) -> None: ...
    def set_render_profile(self, profile: RenderProfile) -> None: ...
    def sequence(self, config: SequenceConfig) -> SequenceRenderer: ...
    def set_trajectory_chunk_windows(self, windows: Sequence[TrajectoryChunkWindow]) -> None: ...
    def render(self, scene: Scene, camera: Camera) -> FrameReport: ...
    def render_image(self, scene: Scene, camera: Camera, width: int, height: int) -> Image: ...
    def render_hdr_image(self, scene: Scene, camera: Camera, config: ImageConfig) -> HdrImage: ...
    def pick(self, x: int, y: int) -> Pick | None: ...
    def profile_frame(self, scene: Scene, camera: Camera, config: ImageConfig) -> FrameTiming: ...
    def install_brick_atlas(self, catalog: BrickCatalog, config: BrickAtlasConfig) -> int: ...
    def stage_brick(self, atlas: int, descriptor: BrickDescriptor, bytes: NDArray[np.uint8]) -> FenceValue: ...
    def evict_brick(self, atlas: int, brick: BrickId) -> FenceValue: ...
    def brick_atlas_metrics(self, atlas: int) -> BrickAtlasMetrics | None: ...
    def request_chunks(self, requests: Sequence[ResidencyRequest]) -> list[ResidencyTicket]: ...
    def deliver_point_chunk(self, catalog: DatasetCatalog, ticket: ResidencyTicket, positions: NDArray[np.float32]) -> None: ...
    def deliver_instance_chunk(self, catalog: DatasetCatalog, ticket: ResidencyTicket, transforms: NDArray[np.float32]) -> None: ...
    def upload_chunks(self, tickets: Sequence[ResidencyTicket]) -> None: ...
    def poll_chunk_uploads(self) -> None: ...
    def set_point_chunk_placements(self, placements: Sequence[PointChunkPlacement]) -> None: ...
    def point_chunk_placement_status(self, id: ChunkPlacementId) -> ChunkPlacementStatus: ...
    def set_instance_chunk_placements(self, placements: Sequence[InstanceChunkPlacement]) -> None: ...
    def instance_chunk_placement_status(self, id: ChunkPlacementId) -> ChunkPlacementStatus: ...
    def resident_generic_chunk(self, ticket: ResidencyTicket) -> ResidentGenericChunk | None: ...
    def set_structure_chunk_placements(self, placements: Sequence[StructureChunkPlacement]) -> None: ...
    def structure_chunk_placement_status(self, id: ChunkPlacementId) -> ChunkPlacementStatus: ...
    def set_bond_chunk_placements(self, placements: Sequence[BondChunkPlacement]) -> None: ...
    def bond_chunk_placement_status(self, id: ChunkPlacementId) -> ChunkPlacementStatus: ...
    def set_relation_chunk_placements(self, placements: Sequence[RelationChunkPlacement]) -> None: ...
    def relation_chunk_placement_status(self, id: ChunkPlacementId) -> ChunkPlacementStatus: ...
    def set_instance_chunk_windows(self, windows: Sequence[InstanceChunkWindow]) -> None: ...
    def set_attribute_chunk_windows(self, windows: Sequence[AttributeChunkWindow]) -> None: ...
    def chunk_residency_metrics(self) -> ChunkResidencyMetrics: ...
    def resident_structure_chunk(self, ticket: ResidencyTicket) -> ResidentStructureChunk | None: ...
    def resident_trajectory_chunk(self, ticket: ResidencyTicket) -> ResidentTrajectoryChunk | None: ...
    def derived_cache_usage(self) -> DerivedCacheUsage: ...

@final
class SequenceFrame:
    ticket: FrameTicket
    def take_image(self) -> Image: ...

@final
class SequenceRenderer:
    pending: int
    def submit(self, engine: Engine, scene: Scene, camera: Camera, timestamp: int) -> FrameTicket: ...
    def poll(self, engine: Engine) -> SequenceFrame | None: ...
    def finish(self, engine: Engine) -> list[SequenceFrame]: ...

@final
class RenderProfile:
    layer_count: int
    @staticmethod
    def inspection() -> RenderProfile: ...
    @staticmethod
    def illustrative() -> RenderProfile: ...
    @staticmethod
    def cinematic() -> RenderProfile: ...
    def with_effect(self, value: PresentationEffect) -> RenderProfile: ...
    def with_layer(self, value: EffectLayer) -> RenderProfile: ...

@final
class ResolvedRenderPlan:
    illustration: IllustrationStyle
    depth_of_field: DepthOfField | None
    motion_blur: MotionBlur | None
    backdrop: BackdropStyle
    lighting: LightingEnvironment
    display: DisplayTransform
    bloom: BloomStyle | None

@final
class BackdropStyle:
    def __new__(cls, top: Rgba8 | None = None, bottom: Rgba8 | None = None, glow_color: Rgba8 | None = None, glow_strength: float = 0.08) -> BackdropStyle: ...
    top: Rgba8
    bottom: Rgba8
    glow_color: Rgba8
    glow_strength: float
    @staticmethod
    def compositing() -> BackdropStyle: ...
    @staticmethod
    def transparent() -> BackdropStyle: ...
    @staticmethod
    def studio() -> BackdropStyle: ...

@final
class LightingEnvironment:
    def __new__(cls, *args: object, **kwargs: object) -> LightingEnvironment: ...
    zenith: Rgba8
    horizon: Rgba8
    ground: Rgba8
    rim_color: Rgba8
    key_color: Rgba8
    fill_color: Rgba8
    key_direction: Vec3
    fill_direction: Vec3
    diffuse_strength: float
    specular_strength: float
    rim_strength: float
    key_strength: float
    fill_strength: float
    key_angular_radius: float
    shadow_strength: float
    @staticmethod
    def neutral() -> LightingEnvironment: ...
    @staticmethod
    def documentary() -> LightingEnvironment: ...

@final
class IllustrationStyle:
    def __new__(cls, silhouette_strength: float = 0.0, cavity_strength: float = 0.0, depth_cue_strength: float = 0.0, posterize_levels: float = 0.0, motion_persistence: float = 0.0, outline_width: float = 0.0) -> IllustrationStyle: ...
    silhouette_strength: float
    cavity_strength: float
    depth_cue_strength: float
    posterize_levels: float
    motion_persistence: float
    outline_width: float
    @staticmethod
    def publication() -> IllustrationStyle: ...

@final
class ToneMapping:
    AcesFitted: ToneMapping
    Disabled: ToneMapping
    Reinhard: ToneMapping
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class DisplayGamut:
    Srgb: DisplayGamut
    DisplayP3: DisplayGamut
    Rec2020: DisplayGamut
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class TransferFunction:
    Srgb: TransferFunction
    Linear: TransferFunction
    Pq: TransferFunction
    Hlg: TransferFunction
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class DisplayTransform:
    def __new__(cls, exposure_ev: float = 0.0, contrast: float = 1.0, saturation: float = 1.0, vignette_strength: float = 0.0, tone_mapping: ToneMapping | None = None, gamut: DisplayGamut | None = None, transfer: TransferFunction | None = None, peak_luminance_nits: float = 100.0) -> DisplayTransform: ...
    exposure_ev: float
    contrast: float
    saturation: float
    vignette_strength: float
    tone_mapping: ToneMapping
    gamut: DisplayGamut
    transfer: TransferFunction
    peak_luminance_nits: float
    @staticmethod
    def cinematic() -> DisplayTransform: ...

@final
class DepthOfField:
    def __new__(cls, focal_length_mm: float = 50.0, f_number: float = 8.0, sensor_width_mm: float = 36.0, max_blur_pixels: float = 14.0, blade_count: int = 7, focus: FocusTarget | None = None) -> DepthOfField: ...
    focal_length_mm: float
    f_number: float
    sensor_width_mm: float
    max_blur_pixels: float
    blade_count: int
    focus: FocusTarget
    @staticmethod
    def cinematic() -> DepthOfField: ...

@final
class MotionBlur:
    def __new__(cls, shutter: float = 0.55, max_blur_pixels: float = 18.0) -> MotionBlur: ...
    shutter: float
    max_blur_pixels: float
    @staticmethod
    def cinematic() -> MotionBlur: ...

@final
class BloomStyle:
    def __new__(cls, threshold: float = 1.7, intensity: float = 0.26, radius: float = 2.0) -> BloomStyle: ...
    threshold: float
    intensity: float
    radius: float
    @staticmethod
    def cinematic() -> BloomStyle: ...

@final
class PresentationEffect:
    @staticmethod
    def illustration(value: IllustrationStyle) -> PresentationEffect: ...
    @staticmethod
    def depth_of_field(value: DepthOfField) -> PresentationEffect: ...
    @staticmethod
    def motion_blur(value: MotionBlur) -> PresentationEffect: ...
    @staticmethod
    def backdrop(value: BackdropStyle) -> PresentationEffect: ...
    @staticmethod
    def lighting(value: LightingEnvironment) -> PresentationEffect: ...
    @staticmethod
    def display(value: DisplayTransform) -> PresentationEffect: ...
    @staticmethod
    def bloom(value: BloomStyle) -> PresentationEffect: ...

@final
class EffectLayer:
    def __new__(cls, effect: PresentationEffect) -> EffectLayer: ...
    def with_weight(self, value: float) -> EffectLayer: ...
    def with_priority(self, value: int) -> EffectLayer: ...

@final
class FocusTarget:
    kind: str
    @staticmethod
    def camera_target() -> FocusTarget: ...
    @staticmethod
    def distance(value: float) -> FocusTarget: ...
    @staticmethod
    def world_point(value: Vec3) -> FocusTarget: ...
    @staticmethod
    def selection(value: SelectionHandle) -> FocusTarget: ...

@final
class RenderSession:
    schema: int
    camera: Camera
    profile: RenderProfile
    @staticmethod
    def capture(scene: Scene, camera: Camera, profile: RenderProfile) -> RenderSession: ...
    @staticmethod
    def from_json(source: str) -> RenderSession: ...
    def to_json(self) -> str: ...
