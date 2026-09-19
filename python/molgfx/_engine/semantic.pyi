"""Focus and context, level of detail, streaming policy and property mapping."""

from typing import Sequence, final

import numpy as np
from numpy.typing import NDArray

from .core import AttributeHandle, RepresentationHandle, RowDomain, ScalarRamp, Scene, SecondaryStructure, SelectionHandle, StructureHandle, SurfaceKind, SurfaceStyle
from .math import Camera, Rgba8, Vec3

__all__ = [
    "PropertyMapping",
    "DifferenceCompositionStyle",
    "DifferenceLayer",
    "DistanceBands",
    "EnsembleCompositionStyle",
    "EnsembleLayer",
    "FocusBand",
    "FocusCompositionStyle",
    "FocusContext",
    "FocusLayer",
    "FocusScene",
    "FocusStyle",
    "FocusSurfaceExtent",
    "FocusView",
    "GenericCompositionScene",
    "LodCluster",
    "SurfaceZone",
    "SurfaceZoneScene",
    "SurfaceZoneStyle",
    "ChunkBounds",
    "ChunkDescriptor",
    "ChunkFootprint",
    "ChunkId",
    "ChunkSpan",
    "DatasetCatalog",
    "DatasetId",
    "LocalRow",
    "LogicalRow",
    "PayloadKind",
    "GenericCompositionView",
    "DeviceLossReport",
    "ResidencyBudget",
    "ResidencyClass",
    "ResidencyEviction",
    "ResidencyFailure",
    "ResidencyKey",
    "ResidencyMachine",
    "ResidencyOutput",
    "ResidencyPhase",
    "ResidencyRequest",
    "ResidencySnapshot",
    "ResidencyTicket",
    "ResidencyUsage",
    "StaleCompletion",
    "ChunkKey",
    "ChunkRequest",
    "LodClusterKey",
    "LodFrame",
    "LodIndex",
    "LodLevel",
    "LodPolicy",
    "LodScene",
    "StreamPlan",
    "StreamPlanner",
    "StreamingBudget",
]

@final
class DatasetId:
    def __new__(cls, value: int) -> DatasetId: ...
    value: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ChunkId:
    def __new__(cls, value: int) -> ChunkId: ...
    value: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class LogicalRow:
    def __new__(cls, value: int) -> LogicalRow: ...
    value: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class LocalRow:
    def __new__(cls, value: int) -> LocalRow: ...
    value: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ChunkFootprint:
    def __new__(cls, source_bytes: int = 0, host_bytes: int = 0, staging_bytes: int = 0, gpu_bytes: int = 0
    ) -> ChunkFootprint: ...
    gpu_bytes: int
    host_bytes: int
    source_bytes: int
    staging_bytes: int
    def total_bytes(self) -> int: ...

@final
class PayloadKind:
    Attribute: PayloadKind
    BondTopology: PayloadKind
    InstanceBatch: PayloadKind
    LabelBrick: PayloadKind
    Mesh: PayloadKind
    PointBatch: PayloadKind
    Proxy: PayloadKind
    RelationBatch: PayloadKind
    ScalarProperty: PayloadKind
    Structure: PayloadKind
    Trajectory: PayloadKind
    VolumeBrick: PayloadKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ChunkSpan:
    def __new__(cls, first: LogicalRow, row_count: int) -> ChunkSpan: ...
    end: LogicalRow
    first: LogicalRow
    row_count: int
    def local_row(self, chunk: ChunkId, row: LogicalRow) -> LocalRow: ...
    def logical_row(self, chunk: ChunkId, row: LocalRow) -> LogicalRow: ...

@final
class ChunkBounds:
    def __new__(cls, min: Sequence[float], max: Sequence[float]) -> ChunkBounds: ...
    max: list[float]
    min: list[float]

@final
class ChunkDescriptor:
    def __new__(cls,
        id: ChunkId,
        parent: ChunkId | None,
        level: int,
        rows: ChunkSpan,
        bounds: ChunkBounds,
        payload_kind: PayloadKind,
        footprint: ChunkFootprint,
    ) -> ChunkDescriptor: ...
    bounds: ChunkBounds
    footprint: ChunkFootprint
    id: ChunkId
    level: int
    parent: ChunkId | None
    payload_kind: PayloadKind
    rows: ChunkSpan

@final
class DatasetCatalog:
    dataset_id: DatasetId
    descriptor_count: int
    ownership: str
    def copy_children(self, parent: ChunkId) -> list[ChunkDescriptor]: ...
    def copy_roots(self) -> list[ChunkDescriptor]: ...
    def descriptor(self, id: ChunkId) -> ChunkDescriptor | None: ...
    @staticmethod
    def from_descriptors(
        dataset_id: DatasetId, descriptors: Sequence[ChunkDescriptor]
    ) -> DatasetCatalog: ...
    def total_footprint(self) -> ChunkFootprint: ...

@final
class GenericCompositionView:
    domains: list[RowDomain]
    normalized_weights: list[float]

@final
class ResidencyBudget:
    def __new__(cls, cpu: int, staging: int, gpu_hot: int, gpu_warm: int, in_flight: int
    ) -> ResidencyBudget: ...
    cpu: int
    gpu_hot: int
    gpu_warm: int
    in_flight: int
    staging: int
    @staticmethod
    def default() -> ResidencyBudget: ...

@final
class ResidencyUsage:
    cpu: int
    gpu_hot: int
    gpu_warm: int
    in_flight: int
    staging: int

@final
class ResidencyClass:
    Hot: ResidencyClass
    Warm: ResidencyClass
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ResidencyPhase:
    Absent: ResidencyPhase
    ReadyCpu: ResidencyPhase
    Requested: ResidencyPhase
    Resident: ResidencyPhase
    Uploading: ResidencyPhase
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ResidencyFailure:
    BudgetExceeded: ResidencyFailure
    InvalidPayload: ResidencyFailure
    Provider: ResidencyFailure
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ResidencyKey:
    def __new__(cls, dataset: DatasetId, chunk: ChunkId, detail: LodLevel) -> ResidencyKey: ...
    chunk: ChunkId
    dataset: DatasetId
    detail: LodLevel
    def __eq__(self, other: object, /) -> bool: ...

@final
class ResidencyTicket:
    generation: int
    key: ResidencyKey
    def __eq__(self, other: object, /) -> bool: ...

@final
class ResidencyRequest:
    def __new__(cls,
        key: ResidencyKey,
        footprint: ChunkFootprint,
        residency_class: ResidencyClass,
        priority: int,
    ) -> ResidencyRequest: ...
    footprint: ChunkFootprint
    key: ResidencyKey
    priority: int
    residency_class: ResidencyClass

@final
class ResidencySnapshot:
    failure: ResidencyFailure | None
    generation: int
    phase: ResidencyPhase

@final
class ResidencyEviction:
    generation: int
    key: ResidencyKey

@final
class StaleCompletion:
    current_generation: int
    current_phase: ResidencyPhase
    ticket: ResidencyTicket

@final
class DeviceLossReport:
    invalidated: int
    ready_cpu: int

@final
class ResidencyOutput:
    def __new__(cls) -> ResidencyOutput: ...
    cancellations: list[ResidencyTicket]
    evictions: list[ResidencyEviction]
    ready_uploads: list[ResidencyTicket]
    requests: list[ResidencyTicket]
    stale: list[StaleCompletion]

@final
class ResidencyMachine:
    def __new__(cls, budget: ResidencyBudget | None = None) -> ResidencyMachine: ...
    budget: ResidencyBudget
    usage: ResidencyUsage
    def begin_upload_into(self, ticket: ResidencyTicket, output: ResidencyOutput) -> bool: ...
    def cancel_into(self, ticket: ResidencyTicket, output: ResidencyOutput) -> bool: ...
    def complete_upload_into(self, ticket: ResidencyTicket, output: ResidencyOutput) -> bool: ...
    def device_lost_into(self, output: ResidencyOutput) -> DeviceLossReport: ...
    def fail_into(
        self, ticket: ResidencyTicket, reason: ResidencyFailure, output: ResidencyOutput
    ) -> bool: ...
    def ready_cpu_into(self, ticket: ResidencyTicket, output: ResidencyOutput) -> bool: ...
    def request_into(
        self, request: ResidencyRequest, output: ResidencyOutput
    ) -> ResidencyTicket: ...
    def set_budget_into(self, budget: ResidencyBudget, output: ResidencyOutput) -> None: ...
    def snapshot(self, key: ResidencyKey) -> ResidencySnapshot: ...

@final
class LodLevel:
    Atom: LodLevel
    Domain: LodLevel
    Residue: LodLevel
    SecondaryStructure: LodLevel
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class LodPolicy:
    def __new__(cls,
        atom_pixels: float = 4.0,
        residue_pixels: float = 1.0,
        secondary_pixels: float = 0.25,
        hysteresis: float = 0.15,
    ) -> LodPolicy: ...
    def select(self, error_pixels: float, importance: float, previous: LodLevel) -> LodLevel: ...
    @staticmethod
    def default() -> LodPolicy: ...

@final
class LodClusterKey:
    index: int
    level: LodLevel
    structure: StructureHandle
    def __eq__(self, other: object, /) -> bool: ...

@final
class LodFrame:
    def __new__(cls) -> LodFrame: ...
    atom_structures: list[StructureHandle]
    levels: list[tuple[StructureHandle, LodLevel]]
    visible: NDArray[np.uint32]
    def level(self, structure: StructureHandle) -> LodLevel | None: ...

@final
class LodIndex:
    def __new__(cls, scene: Scene) -> LodIndex: ...
    cluster_count: int
    def select_into(
        self,
        camera: Camera,
        viewport: tuple[int, int],
        output: LodFrame,
        policy: LodPolicy | None = None,
        previous: LodFrame | None = None,
    ) -> None: ...

@final
class LodScene:
    def __new__(cls) -> LodScene: ...
    coarse_representation_count: int
    detail_representation_count: int
    primitive_count: int
    def apply(self, scene: Scene, index: LodIndex, frame: LodFrame) -> None: ...
    def apply_transition(
        self,
        scene: Scene,
        index: LodIndex,
        from_frame: LodFrame,
        to_frame: LodFrame,
        weight: float,
    ) -> None: ...
    def bind_coarse_representation(
        self, scene: Scene, structure: StructureHandle, representation: RepresentationHandle
    ) -> None: ...
    def bind_detail_representation(
        self, scene: Scene, structure: StructureHandle, representation: RepresentationHandle
    ) -> None: ...
    def unbind_coarse_representation(
        self, scene: Scene, representation: RepresentationHandle
    ) -> None: ...
    def unbind_detail_representation(
        self, scene: Scene, representation: RepresentationHandle
    ) -> None: ...

@final
class StreamingBudget:
    def __new__(cls, max_resident_bytes: int = 268_435_456, max_requests_per_frame: int = 64
    ) -> StreamingBudget: ...
    max_requests_per_frame: int
    max_resident_bytes: int
    @staticmethod
    def default() -> StreamingBudget: ...

@final
class ChunkKey:
    index: int
    level: LodLevel
    structure: StructureHandle

@final
class ChunkRequest:
    bytes: int
    key: ChunkKey
    priority: float

@final
class StreamPlan:
    def __new__(cls) -> StreamPlan: ...
    evict: list[ChunkKey]
    resident_bytes: int
    retain: list[ChunkRequest]

@final
class StreamPlanner:
    def __new__(cls, budget: StreamingBudget | None = None) -> StreamPlanner: ...
    budget: StreamingBudget
    def copy_plan_from_numpy(
        self,
        structure: StructureHandle,
        levels: NDArray[np.uint8],
        indices: NDArray[np.uint32],
        priorities: NDArray[np.float32],
        bytes: NDArray[np.uint64],
        output: StreamPlan,
    ) -> None: ...

@final
class PropertyMapping:
    def __new__(cls, domain: Sequence[float], visual: Sequence[float]) -> PropertyMapping: ...
    def map(self, value: float) -> float: ...
    def unmap(self, value: float) -> float: ...

@final
class LodCluster:
    def __new__(cls,
        structure: StructureHandle,
        level: LodLevel,
        index: int,
        center: Vec3,
        radius: float,
        importance: float,
        atom_count: int,
    ) -> LodCluster: ...
    atom_count: int
    center: Vec3
    importance: float
    key: LodClusterKey
    radius: float

@final
class SurfaceZone:
    representation: RepresentationHandle
    selection: SelectionHandle

@final
class SurfaceZoneStyle:
    def __new__(cls,
        distance: float = 5.0,
        kind: SurfaceKind = SurfaceKind.SolventExcluded,
        presentation: SurfaceStyle = SurfaceStyle.Solid,
        opacity: float = 0.35,
        color: Rgba8 | None = None,
    ) -> SurfaceZoneStyle: ...
    @staticmethod
    def default() -> SurfaceZoneStyle: ...

@final
class SurfaceZoneScene:
    @staticmethod
    def surface_components(
        scene: Scene, surface: SelectionHandle, minimum_atoms: int
    ) -> SurfaceZone: ...
    @staticmethod
    def surface_zone(
        scene: Scene, surface: SelectionHandle, around: SelectionHandle
    ) -> SurfaceZone: ...
    @staticmethod
    def surface_zone_with(
        scene: Scene,
        surface: SelectionHandle,
        around: SelectionHandle,
        style: SurfaceZoneStyle,
    ) -> SurfaceZone: ...


@final
class FocusLayer:
    def __new__(cls, domain: RowDomain, emphasis: AttributeHandle) -> FocusLayer: ...
    domain: RowDomain
    emphasis: AttributeHandle

@final
class FocusCompositionStyle:
    def __new__(cls, context_opacity: float = 0.16, focus_opacity: float = 1.0, order: int = 0
    ) -> FocusCompositionStyle: ...
    @staticmethod
    def default() -> FocusCompositionStyle: ...

@final
class DifferenceLayer:
    def __new__(cls, domain: RowDomain, delta: AttributeHandle) -> DifferenceLayer: ...
    domain: RowDomain
    delta: AttributeHandle

@final
class DifferenceCompositionStyle:
    def __new__(cls,
        context_threshold: float,
        emphasis_threshold: float,
        context_opacity: float,
        ramp: ScalarRamp,
        order: int,
    ) -> DifferenceCompositionStyle: ...

@final
class EnsembleLayer:
    def __new__(cls, domain: RowDomain, weight: float, color: Rgba8) -> EnsembleLayer: ...
    domain: RowDomain
    weight: float
    color: Rgba8

@final
class EnsembleCompositionStyle:
    def __new__(cls,
        dominant_opacity: float = 1.0,
        alternate_opacity: float = 0.55,
        minimum_opacity: float = 0.08,
        order: int = 0,
    ) -> EnsembleCompositionStyle: ...
    @staticmethod
    def default() -> EnsembleCompositionStyle: ...

@final
class GenericCompositionScene:
    @staticmethod
    def compose_focus(
        scene: Scene, layer: FocusLayer, style: FocusCompositionStyle
    ) -> GenericCompositionView: ...
    @staticmethod
    def compose_difference(
        scene: Scene, layers: Sequence[DifferenceLayer], style: DifferenceCompositionStyle
    ) -> GenericCompositionView: ...
    @staticmethod
    def compose_ensemble(
        scene: Scene, layers: Sequence[EnsembleLayer], style: EnsembleCompositionStyle
    ) -> GenericCompositionView: ...

@final
class DistanceBands:
    def __new__(cls, near: float, mid: float) -> DistanceBands: ...
    near: float
    mid: float
    @staticmethod
    def default() -> DistanceBands: ...
    def classify(self, distance: float) -> FocusBand: ...

@final
class FocusBand:
    Near: FocusBand
    Mid: FocusBand
    Far: FocusBand
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class FocusContext:
    Cartoon: FocusContext
    Trace: FocusContext
    Tube: FocusContext
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class FocusSurfaceExtent:
    Pocket: FocusSurfaceExtent
    Structure: FocusSurfaceExtent
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class FocusStyle:
    def __new__(cls,
        bands: DistanceBands | None = None,
        pocket_opacity: float = 0.34,
        context_opacity: float = 0.3,
        pocket_presentation: SurfaceStyle | None = None,
        surface_extent: FocusSurfaceExtent | None = None,
        pocket_color: Rgba8 | None = None,
        context_geometry: FocusContext | None = None,
        context_color: Rgba8 | None = None,
        solvent_opacity: float = 0.42,
        solvent_color: Rgba8 | None = None,
    ) -> FocusStyle: ...
    @staticmethod
    def default() -> FocusStyle: ...

@final
class FocusView:
    focus: SelectionHandle
    near: SelectionHandle
    pocket: SelectionHandle
    mid: SelectionHandle
    context: SelectionHandle
    solvent: SelectionHandle
    focus_representation: RepresentationHandle
    near_representation: RepresentationHandle
    mid_representation: RepresentationHandle
    pocket_representation: RepresentationHandle
    context_representation: RepresentationHandle

@final
class FocusScene:
    @staticmethod
    def focus(scene: Scene, selection: SelectionHandle) -> FocusView: ...
    @staticmethod
    def focus_with(
        scene: Scene, selection: SelectionHandle, style: FocusStyle | None = None
    ) -> FocusView: ...
