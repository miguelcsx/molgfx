"""The scene graph and everything a program edits.

Structures, selections, representations, columns, overlays, timelines and the
handles that name them, plus the constants that bound a column.
"""

import os
from typing import Sequence, final

import numpy as np
from numpy.typing import NDArray

from .math import Aabb, Camera, Mat4, Quat, Rgba8, Vec3
from .semantic import GenericCompositionView, ResidencyTicket

__all__ = [
    "ObjectIdentity",
    "EntityDescription",
    "AnchorDescription",
    "SelectionMask",
    "TargetDescription",
    "RowDomainDescription",
    "SourceRowsDescription",
    "SceneDescription",
    "StructureDescription",
    "SelectionDescription",
    "TableCounts",
    "RepresentationDescription",
    "SurfaceComponentDescription",
    "VisualStyleDescription",
    "VisualAttributeDescription",
    "VisualInstructionDescription",
    "ColorDescription",
    "MaterialDescription",
    "ClipDescription",
    "VolumeDescription",
    "OccupancyDescription",
    "VolumeStyleDescription",
    "VolumeTransferPointDescription",
    "SegmentationStyleDescription",
    "SegmentStyleDescription",
    "RegionDescription",
    "ScalarSemanticsDescription",
    "AtomPropertyDescription",
    "PropertyAppearanceDescription",
    "SurfaceScalarDescription",
    "PointBatchDescription",
    "InstanceBatchDescription",
    "AttributeDescription",
    "RelationBatchDescription",
    "DomainVisualDescription",
    "GuideStyleDescription",
    "GuideDescription",
    "MarkerStyleDescription",
    "AnnotationDescription",
    "MeasurementDescription",
    "InteractionDescription",
    "LigandPoseDescription",
    "LigandPoseBatchDescription",
    "ContentAddress",
    "ReferencedPayloadKind",
    "PayloadReference",
    "SceneManifest",
    "Annotation",
    "AnnotationAnchor",
    "AnnotationKind",
    "MarkerShape",
    "MarkerStyle",
    "Measurement",
    "MeasurementKind",
    "GuideCap",
    "GuideStyle",
    "MeshTopology",
    "Mesh",
    "MeshInstance",
    "MeshVertex",
    "ParticleShape",
    "ArcballController",
    "Button",
    "FlyController",
    "InputEvent",
    "Key",
    "OrbitController",
    "AtomProperty",
    "AtomPropertyMeaning",
    "RelationPattern",
    "RelationStyle",
    "RowDomain",
    "AnalyticCapsule",
    "AnalyticSphere",
    "AnalyticTemplate",
    "AnnotationHandle",
    "AtomPropertyHandle",
    "AttributeHandle",
    "EntityKind",
    "EntityRef",
    "Ensemble",
    "EnsembleHandle",
    "GuideHandle",
    "InstanceBatchHandle",
    "InteractionHandle",
    "LigandPoseBatchHandle",
    "MeasurementHandle",
    "MeshHandle",
    "MeshInstanceHandle",
    "OverlayHandle",
    "PointBatchHandle",
    "PrimitiveHandle",
    "RelationBatchHandle",
    "RepresentationHandle",
    "SegmentationHandle",
    "SelectionHandle",
    "StructureHandle",
    "TimelineTrackHandle",
    "VolumeHandle",
    "VolumeSegmentRef",
    "InteractionEdge",
    "InteractionAnchor",
    "InteractionDirection",
    "InteractionGeometry",
    "InteractionKind",
    "InteractionPattern",
    "InteractionStyle",
    "LicoriceTemplate",
    "LigandPose",
    "LigandPoseBatch",
    "InstanceBatch",
    "InstanceStyle",
    "RigidInstance",
    "Particle",
    "PointBatch",
    "PointGlyph",
    "PointStyle",
    "Primitive",
    "AttributeColumn",
    "AttributeDescriptor",
    "AttributeKind",
    "AttributeValues",
    "AnchorLayout",
    "Relation",
    "RelationBatch",
    "RelationDependency",
    "RelationLayout",
    "RelationPartition",
    "RowEntityRef",
    "SpatialAnchor",
    "TemplatePartRef",
    "Provenance",
    "PropertyAppearance",
    "PropertyAppearanceSample",
    "Representation",
    "RepresentationKind",
    "RepresentationPreset",
    "RepresentationTarget",
    "Scene",
    "PropertyComparison",
    "SecondaryStructure",
    "Select",
    "Material",
    "MaterialModel",
    "OccupancyStream",
    "ScalarVolume",
    "SegmentedVolume",
    "MemoryOwnership",
    "MemoryTransferExclusion",
    "OverlayAnchor",
    "MeshDescription",
    "MeshInstanceDescription",
    "OverlayDescription",
    "ParticleMotionDescription",
    "PrimitiveDescription",
    "AnisotropicEllipsoid",
    "CarbohydrateShape",
    "CarbohydrateSymbol",
    "Guide",
    "PlanarRegion",
    "Quadric",
    "OverlayContent",
    "OverlayKind",
    "ScreenOverlay",
    "ParticleBoundary",
    "ParticleMotion",
    "FaceVisibility",
    "PropertyLegend",
    "SurfaceComponentPolicy",
    "SurfaceComponentThreshold",
    "EntityProvenance",
    "ProvenanceDetail",
    "RepresentationConfig",
    "RepresentationInput",
    "RepresentationParams",
    "AtomSelection",
    "VisualInstructionGpu",
    "VolumeRendering",
    "VolumeTransferFunction",
    "VolumeTransferPoint",
    "CameraBookmark",
    "CameraEasing",
    "CameraKeyframe",
    "CameraPath",
    "PlaybackMode",
    "TimeWarp",
    "Timeline",
    "BondTopologyFrame",
    "BondTopologySegment",
    "TopologyBond",
    "TrajectoryBranch",
    "TrajectoryChunkWindow",
    "TrajectoryFrame",
    "TrajectorySegment",
    "TrajectoryStateGraph",
    "ValidationKind",
    "ValidationMarker",
    "ColorScheme",
    "ScalarContours",
    "ScalarFieldSemantics",
    "ScalarRamp",
    "SurfaceKind",
    "SurfaceScalarOverlay",
    "SurfaceStyle",
    "ClipCap",
    "ClipPlane",
    "ClipSet",
    "CrystalCell",
    "SegmentStyle",
    "SegmentStyleTable",
    "SegmentationStyle",
    "SymmetryInstance",
    "TubeRadiusMapping",
    "VolumeStyle",
    "VisualCompatibility",
    "VisualOutput",
    "VisualStage",
    "BoolExpr",
    "ColorExpr",
    "ColorParameter",
    "ScalarExpr",
    "ScalarParameter",
    "VectorExpr",
    "VectorParameter",
    "VisualAttributeRef",
    "VisualColumnKey",
    "VisualDescriptor",
    "VisualEvaluation",
    "VisualInputs",
    "VisualProgram",
    "VisualProgramBuilder",
    "VisualStyle",
    "cpk_color",
    "select",
    "vdw_radius",
    "read_manifest",
    "write_manifest",
    "MAX_CLIP_PLANES",
    "MAX_MESH_VERTICES",
    "MAX_VISUAL_INSTRUCTIONS",
    "MAX_VISUAL_PARAMETERS",
    "MAX_VISUAL_PROPERTIES",
    "MAX_VOLUME_TRANSFER_POINTS",
]

@final
class AnnotationAnchor:
    position: Vec3
    source_entity: EntityRef | None
    @staticmethod
    def entity(position: Vec3, entity: EntityRef) -> AnnotationAnchor: ...
    @staticmethod
    def world(position: Vec3) -> AnnotationAnchor: ...

@final
class MarkerShape:
    Circle: MarkerShape
    Crosshair: MarkerShape
    Diamond: MarkerShape
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class MarkerStyle:
    def __new__(cls, color: Rgba8 | None = None, radius_pixels: float = 6.0, shape: MarkerShape | None = None) -> MarkerStyle: ...
    color: Rgba8
    radius_pixels: float
    shape: MarkerShape

@final
class AnnotationKind:
    Hypothesis: AnnotationKind
    Marker: AnnotationKind
    Note: AnnotationKind
    Region: AnnotationKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Annotation:
    anchor: AnnotationAnchor | None
    kind: AnnotationKind
    marker_style: MarkerStyle
    owner: StructureHandle
    priority: int
    region_selection: SelectionHandle | None
    text: str
    visible: bool
    @staticmethod
    def hypothesis(owner: StructureHandle, anchor: AnnotationAnchor, text: str) -> Annotation: ...
    @staticmethod
    def marker(owner: StructureHandle, anchor: AnnotationAnchor, style: MarkerStyle) -> Annotation: ...
    @staticmethod
    def note(owner: StructureHandle, anchor: AnnotationAnchor, text: str) -> Annotation: ...
    @staticmethod
    def region(owner: StructureHandle, selection: SelectionHandle, label: str) -> Annotation: ...
    def with_priority(self, priority: int) -> Annotation: ...

@final
class MeasurementKind:
    Angle: MeasurementKind
    Dihedral: MeasurementKind
    Distance: MeasurementKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Measurement:
    anchors: list[AnnotationAnchor]
    kind: MeasurementKind
    label: str
    owner: StructureHandle
    priority: int
    provenance: str
    value: float
    visible: bool
    @staticmethod
    def angle(owner: StructureHandle, anchors: Sequence[AnnotationAnchor], value: float, provenance: str) -> Measurement: ...
    @staticmethod
    def dihedral(owner: StructureHandle, anchors: Sequence[AnnotationAnchor], value: float, provenance: str) -> Measurement: ...
    @staticmethod
    def distance(owner: StructureHandle, anchors: Sequence[AnnotationAnchor], value: float, provenance: str) -> Measurement: ...
    def with_priority(self, priority: int) -> Measurement: ...

@final
class ParticleShape:
    Box: ParticleShape
    Circle: ParticleShape
    Cylinder: ParticleShape
    Gaussian: ParticleShape
    Sphere: ParticleShape
    Spherocylinder: ParticleShape
    Square: ParticleShape
    Superquadric: ParticleShape
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Mesh:
    owner: StructureHandle
    material: Material
    clipping: ClipSet
    face_visibility: FaceVisibility
    component_policy: SurfaceComponentPolicy
    visible: bool
    bounds: Aabb
    vertex_count: int
    triangle_count: int
    def vertex(self, index: int) -> MeshVertex | None: ...
    def copy_positions_numpy(self) -> NDArray[np.float32]: ...
    def copy_normals_numpy(self) -> NDArray[np.float32]: ...
    def copy_colors_numpy(self) -> NDArray[np.uint8]: ...
    def copy_indices_numpy(self) -> NDArray[np.uint32]: ...

@final
class MeshInstance:
    mesh: MeshHandle
    transform: Mat4
    visible: bool

@final
class MeshVertex:
    position: Vec3
    normal: Vec3
    color: Rgba8

@final
class MeshTopology:
    Quads: MeshTopology
    TriangleFan: MeshTopology
    TriangleStrip: MeshTopology
    Triangles: MeshTopology
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class GuideCap:
    Arrow: GuideCap
    DoubleArrow: GuideCap
    Plain: GuideCap
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class GuideStyle:
    def __new__(cls, color: Rgba8, pattern: RelationPattern = RelationPattern.Solid, width_pixels: float = 1.6, opacity: float = 1.0, period_pixels: float = 8.0, duty_cycle: float = 0.5, cap: GuideCap = GuideCap.Plain, arrow_pixels: float = 9.0) -> GuideStyle: ...

@final
class Button:
    Left: Button
    Middle: Button
    Right: Button
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Key:
    Backward: Key
    Down: Key
    Forward: Key
    Left: Key
    Right: Key
    Up: Key
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class InputEvent:
    @staticmethod
    def key(key: Key, pressed: bool) -> InputEvent: ...
    @staticmethod
    def pinch(scale: float) -> InputEvent: ...
    @staticmethod
    def pointer_button(button: Button, pressed: bool, x: float, y: float) -> InputEvent: ...
    @staticmethod
    def pointer_move(x: float, y: float) -> InputEvent: ...
    @staticmethod
    def scroll(delta: float) -> InputEvent: ...

@final
class ArcballController:
    def __new__(cls) -> ArcballController: ...
    def update(self, event: InputEvent, camera: Camera) -> None: ...

@final
class OrbitController:
    def __new__(cls) -> OrbitController: ...
    def update(self, event: InputEvent, camera: Camera) -> None: ...

@final
class FlyController:
    def __new__(cls) -> FlyController: ...
    def advance(self, camera: Camera, dt: float, speed: float) -> None: ...
    def update(self, event: InputEvent, camera: Camera) -> None: ...

@final
class MemoryOwnership:
    Copied: MemoryOwnership
    Shared: MemoryOwnership
    Transferred: MemoryOwnership
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class MemoryTransferExclusion:
    DlpackLifetimeUnavailable: MemoryTransferExclusion
    RustStorageNotMovable: MemoryTransferExclusion
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class OverlayAnchor:
    def __new__(cls, normalized: tuple[float, float], pixels: tuple[float, float] = ...) -> OverlayAnchor: ...
    normalized: tuple[float, float]
    pixels: tuple[float, float]

@final
class CameraEasing:
    Linear: CameraEasing
    SmoothStep: CameraEasing
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class CameraKeyframe:
    def __new__(cls, time_seconds: float, camera: Camera) -> CameraKeyframe: ...
    camera: Camera
    time_seconds: float

@final
class CameraPath:
    def __new__(cls, keyframes: Sequence[CameraKeyframe], easing: CameraEasing = CameraEasing.SmoothStep) -> CameraPath: ...
    keyframes: list[CameraKeyframe]
    range: tuple[float, float]
    def sample(self, time_seconds: float) -> Camera | None: ...

@final
class CameraBookmark:
    def __new__(cls, label: str, time_seconds: float, camera: Camera) -> CameraBookmark: ...
    camera: Camera
    label: str
    time_seconds: float
    @staticmethod
    def from_json(source: str) -> CameraBookmark: ...
    def restore(self, timeline: Timeline, scene: Scene, camera: Camera) -> None: ...
    def to_json(self) -> str: ...

@final
class PlaybackMode:
    Clamp: PlaybackMode
    Loop: PlaybackMode
    PingPong: PlaybackMode
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class TimeWarp:
    def __new__(cls, global_origin: float, local_origin: float, rate: float, range: tuple[float, float], playback: PlaybackMode) -> TimeWarp: ...
    range: tuple[float, float]
    def phase(self, global_seconds: float) -> float | None: ...
    def sample(self, global_seconds: float) -> float | None: ...

@final
class Timeline:
    def __new__(cls) -> Timeline: ...
    time_seconds: float
    def apply(self, scene: Scene, global_seconds: float) -> None: ...
    def bind_bond_topology(self, scene: Scene, structure: StructureHandle, warp: TimeWarp) -> TimelineTrackHandle: ...
    def bind_instances_from_numpy(self, scene: Scene, batch: InstanceBatchHandle, start_translations: NDArray[np.float32], start_orientations: NDArray[np.float32], start_scales: NDArray[np.float32], end_translations: NDArray[np.float32], end_orientations: NDArray[np.float32], end_scales: NDArray[np.float32], warp: TimeWarp) -> TimelineTrackHandle: ...
    def bind_points_from_numpy(self, scene: Scene, batch: PointBatchHandle, start: NDArray[np.float32], end: NDArray[np.float32], warp: TimeWarp) -> TimelineTrackHandle: ...
    def bind_scalar_attribute_from_numpy(self, scene: Scene, attribute: AttributeHandle, start: NDArray[np.float32], end: NDArray[np.float32], warp: TimeWarp) -> TimelineTrackHandle: ...
    def bind_trajectory(self, scene: Scene, structure: StructureHandle, warp: TimeWarp) -> TimelineTrackHandle: ...
    def bind_vector_attribute_from_numpy(self, scene: Scene, attribute: AttributeHandle, start: NDArray[np.float32], end: NDArray[np.float32], warp: TimeWarp) -> TimelineTrackHandle: ...
    def remove(self, scene: Scene, handle: TimelineTrackHandle) -> bool: ...
    def __len__(self) -> int: ...

@final
class TopologyBond:
    def __new__(cls, atom_a: int, atom_b: int, aromatic: bool = False) -> TopologyBond: ...
    aromatic: bool
    atoms: tuple[int, int]

@final
class BondTopologyFrame:
    def __new__(cls, index: int, time_seconds: float, atom_count: int, bonds: Sequence[TopologyBond], provenance: str) -> BondTopologyFrame: ...
    aromatic: NDArray[np.bool_]
    atom_count: int
    atom_pairs: NDArray[np.uint32]
    index: int
    provenance: str
    time_seconds: float

@final
class BondTopologySegment:
    def __new__(cls, start: BondTopologyFrame, end: BondTopologyFrame, sample_seconds: float) -> BondTopologySegment: ...
    aromatic: NDArray[np.bool_]
    atom_pairs: NDArray[np.uint32]
    end: BondTopologyFrame
    interpolation: float
    sample_seconds: float
    start: BondTopologyFrame
    weights: NDArray[np.float32]
    def set_sample_time(self, sample_seconds: float) -> None: ...

@final
class TrajectoryChunkWindow:
    def __new__(cls, structure: ResidencyTicket, start: ResidencyTicket, end: ResidencyTicket, interpolation: float) -> TrajectoryChunkWindow: ...
    end: ResidencyTicket
    interpolation: float
    start: ResidencyTicket
    structure: ResidencyTicket

@final
class TrajectoryFrame:
    index: int
    provenance: str
    time_seconds: float
    def aabb(self) -> Aabb: ...
    def copy_positions_numpy(self) -> NDArray[np.float32]: ...
    @staticmethod
    def copy_from_numpy(index: int, time_seconds: float, positions: NDArray[np.float32], provenance: str) -> TrajectoryFrame: ...

@final
class TrajectorySegment:
    def __new__(cls, start: TrajectoryFrame, end: TrajectoryFrame, sample_seconds: float) -> TrajectorySegment: ...
    atom_count: int
    end: TrajectoryFrame
    interpolation: float
    sample_seconds: float
    start: TrajectoryFrame
    def set_sample_time(self, sample_seconds: float) -> None: ...
    def union_aabb(self) -> Aabb: ...

@final
class TrajectoryBranch:
    def __new__(cls, from_state: int, to_state: int, event: str, probability: float = 1.0) -> TrajectoryBranch: ...
    event: str
    from_state: int
    probability: float
    to_state: int

@final
class TrajectoryStateGraph:
    def __new__(cls, states: Sequence[int], branches: Sequence[TrajectoryBranch], initial: int) -> TrajectoryStateGraph: ...
    active: int
    branches: list[TrajectoryBranch]
    states: NDArray[np.uint32]
    def seek(self, scene: Scene, structure: StructureHandle, state: int, segment: TrajectorySegment) -> None: ...
    def target(self, event: str) -> int | None: ...
    def transition(self, scene: Scene, structure: StructureHandle, event: str, segment: TrajectorySegment) -> int: ...

@final
class ValidationKind:
    Clash: ValidationKind
    Density: ValidationKind
    Geometry: ValidationKind
    Other: ValidationKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ValidationMarker:
    def __new__(cls, owner: StructureHandle, anchor: AnnotationAnchor, kind: ValidationKind, severity: float, style: MarkerStyle) -> ValidationMarker: ...
    severity: float

@final
class ColorScheme:
    kind: str
    @staticmethod
    def by_chain() -> ColorScheme: ...
    @staticmethod
    def by_element() -> ColorScheme: ...
    @staticmethod
    def by_residue() -> ColorScheme: ...
    @staticmethod
    def by_secondary_structure() -> ColorScheme: ...
    @staticmethod
    def uniform(color: Rgba8) -> ColorScheme: ...

@final
class SurfaceKind:
    Gaussian: SurfaceKind
    SolventAccessible: SurfaceKind
    SolventExcluded: SurfaceKind
    VanDerWaals: SurfaceKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class SurfaceStyle:
    Contour: SurfaceStyle
    Dots: SurfaceStyle
    FilledContour: SurfaceStyle
    Mesh: SurfaceStyle
    SoftUnion: SurfaceStyle
    Solid: SurfaceStyle
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ScalarFieldSemantics:
    kind: str
    @staticmethod
    def quantity(name: str, units: str, provenance: str) -> ScalarFieldSemantics: ...
    @staticmethod
    def uncalibrated_rank() -> ScalarFieldSemantics: ...

@final
class ScalarRamp:
    def __new__(cls, values: tuple[float, float, float], colors: tuple[Rgba8, Rgba8, Rgba8]) -> ScalarRamp: ...
    colors: tuple[Rgba8, Rgba8, Rgba8]
    values: tuple[float, float, float]
    @staticmethod
    def diverging(extent: float) -> ScalarRamp: ...
    def sample(self, value: float, missing: Rgba8) -> Rgba8: ...
    @staticmethod
    def sequential(domain: tuple[float, float]) -> ScalarRamp: ...

@final
class ScalarContours:
    def __new__(cls, interval: float, width_pixels: float) -> ScalarContours: ...
    interval: float
    width_pixels: float

@final
class SurfaceScalarOverlay:
    def __new__(cls, field: VolumeHandle, ramp: ScalarRamp) -> SurfaceScalarOverlay: ...
    field: VolumeHandle
    ramp: ScalarRamp
    def with_contours(self, contours: ScalarContours) -> SurfaceScalarOverlay: ...

@final
class VolumeStyle:
    @staticmethod
    def direct() -> VolumeStyle: ...
    @staticmethod
    def isosurface() -> VolumeStyle: ...
    @staticmethod
    def liquid_surface() -> VolumeStyle: ...
    @staticmethod
    def medium() -> VolumeStyle: ...
    def region(self, minimum: tuple[int, int, int], maximum: tuple[int, int, int], dimensions: tuple[int, int, int]) -> VolumeStyle: ...
    def sampling(self, opacity_scale: float, step_scale: float) -> VolumeStyle: ...
    @staticmethod
    def slice(plane: ClipPlane) -> VolumeStyle: ...

@final
class ClipCap:
    Open: ClipCap
    Solid: ClipCap
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ClipPlane:
    normal: Vec3
    offset: float
    @staticmethod
    def from_point_normal(point: Vec3, normal: Vec3) -> ClipPlane: ...
    def reversed(self) -> ClipPlane: ...
    def signed_distance(self, point: Vec3) -> float: ...

@final
class ClipSet:
    def __new__(cls, planes: Sequence[ClipPlane]) -> ClipSet: ...
    cap: ClipCap
    plane_count: int
    def contains(self, point: Vec3) -> bool: ...
    @staticmethod
    def slab(center: Vec3, normal: Vec3, thickness: float) -> ClipSet: ...
    def with_cap(self, cap: ClipCap) -> ClipSet: ...

@final
class SegmentStyle:
    def __new__(cls, label: int, color: Rgba8, opacity: float) -> SegmentStyle: ...
    color: Rgba8
    label: int
    opacity: float

@final
class SegmentStyleTable:
    def __new__(cls, styles: Sequence[SegmentStyle]) -> SegmentStyleTable: ...
    len: int
    @staticmethod
    def default() -> SegmentStyleTable: ...
    def style_for(self, label: int) -> SegmentStyle | None: ...

@final
class SegmentationStyle:
    def __new__(cls, styles: SegmentStyleTable | None = None, opacity_scale: float = 1.0, step_scale: float = 0.65) -> SegmentationStyle: ...
    opacity_scale: float
    step_scale: float
    styles: SegmentStyleTable
    @staticmethod
    def default() -> SegmentationStyle: ...

@final
class TubeRadiusMapping:
    @staticmethod
    def b_factor(domain: tuple[float, float], radii: tuple[float, float]) -> TubeRadiusMapping: ...
    @staticmethod
    def constant() -> TubeRadiusMapping: ...
    def radius(self, value: float, fallback: float) -> float: ...
    def value(self, radius: float) -> float | None: ...

@final
class CrystalCell:
    def __new__(cls, lengths: tuple[float, float, float], angles_degrees: tuple[float, float, float]) -> CrystalCell: ...
    angles_degrees: tuple[float, float, float]
    lengths: tuple[float, float, float]
    origin: Vec3
    def with_origin(self, origin: Vec3) -> CrystalCell: ...

@final
class SymmetryInstance:
    def __new__(cls, id: int, transform: Mat4) -> SymmetryInstance: ...
    id: int
    transform: Mat4

@final
class AtomPropertyMeaning:
    Charge: AtomPropertyMeaning
    Confidence: AtomPropertyMeaning
    Exposure: AtomPropertyMeaning
    Flexibility: AtomPropertyMeaning
    Generic: AtomPropertyMeaning
    Hydrophobicity: AtomPropertyMeaning
    LocalResolution: AtomPropertyMeaning
    Occupancy: AtomPropertyMeaning
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class AtomProperty:
    numpy_ownership: MemoryOwnership
    @staticmethod
    def copy_from_numpy(owner: StructureHandle, name: str, values: NDArray[np.float32], meaning: AtomPropertyMeaning, semantics: ScalarFieldSemantics) -> AtomProperty: ...
    def copy_values(self) -> NDArray[np.float32]: ...

@final
class RelationPattern:
    Dashed: RelationPattern
    Dotted: RelationPattern
    Solid: RelationPattern
    Spring: RelationPattern
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class RowDomain:
    @staticmethod
    def atoms(handle: StructureHandle) -> RowDomain: ...
    @staticmethod
    def instances(handle: InstanceBatchHandle) -> RowDomain: ...
    @staticmethod
    def points(handle: PointBatchHandle) -> RowDomain: ...
    @staticmethod
    def relations(handle: RelationBatchHandle) -> RowDomain: ...
    @staticmethod
    def template_parts(handle: InstanceBatchHandle) -> RowDomain: ...

@final
class AttributeColumn:
    descriptor: AttributeDescriptor
    domain: RowDomain
    fingerprint: int
    is_empty: bool
    kind: AttributeKind
    len: int
    name: str
    stride: int
    values: AttributeValues

@final
class AttributeDescriptor:
    name: str
    provenance: str | None
    quantity: str | None
    unit: str | None

@final
class AttributeKind:
    Scalar: AttributeKind
    Category: AttributeKind
    Vector: AttributeKind
    Color: AttributeKind
    stride: int
    def __eq__(self, other: object, /) -> bool: ...

@final
class AttributeValues:
    is_empty: bool
    kind: AttributeKind
    len: int
    def copy_categories_numpy(self) -> NDArray[np.uint32] | None: ...
    def copy_colors_numpy(self) -> NDArray[np.uint8] | None: ...
    def copy_scalars_numpy(self) -> NDArray[np.float32] | None: ...
    def copy_vectors_numpy(self) -> NDArray[np.float32] | None: ...

@final
class AnchorLayout:
    World: AnchorLayout
    Atom: AnchorLayout
    Point: AnchorLayout
    Instance: AnchorLayout
    TemplatePart: AnchorLayout
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Relation:
    def __new__(cls, start: SpatialAnchor, end: SpatialAnchor) -> Relation: ...
    start: SpatialAnchor
    end: SpatialAnchor
    layout: RelationLayout

@final
class RelationBatch:
    relations: list[Relation]
    style: RelationStyle
    partitions: list[RelationPartition]
    dependencies: list[RelationDependency]
    visible: bool
    row_count: int
    def copy_remap_numpy(self) -> NDArray[np.uint32] | None: ...

@final
class RelationDependency:
    domain: RowDomain
    maximum_row: int

@final
class RelationLayout:
    def __new__(cls, start: AnchorLayout, end: AnchorLayout) -> RelationLayout: ...
    start: AnchorLayout
    end: AnchorLayout
    is_dynamic: bool
    def __repr__(self) -> str: ...

@final
class RelationPartition:
    layout: RelationLayout
    rows: tuple[int, int]

@final
class RowEntityRef:
    def __new__(cls, domain: RowDomain, row: int) -> RowEntityRef: ...
    domain: RowDomain
    row: int

@final
class SpatialAnchor:
    @staticmethod
    def world(position: Vec3) -> SpatialAnchor: ...
    @staticmethod
    def entity(entity: RowEntityRef) -> SpatialAnchor: ...
    @staticmethod
    def template_part(reference: TemplatePartRef) -> SpatialAnchor: ...
    position: Vec3 | None
    source_entity: RowEntityRef | None
    reference: TemplatePartRef | None
    layout: AnchorLayout
    is_dynamic: bool
    source_domain: RowDomain | None
    def __repr__(self) -> str: ...

@final
class TemplatePartRef:
    def __new__(cls, batch: InstanceBatchHandle, instance_row: int, part_row: int) -> TemplatePartRef: ...
    batch: InstanceBatchHandle
    instance_row: int
    part_row: int

@final
class AnalyticCapsule:
    def __new__(cls, start: Vec3, end: Vec3, radius: float) -> AnalyticCapsule: ...
    end: Vec3
    radius: float
    start: Vec3
    def __repr__(self) -> str: ...

@final
class AnalyticSphere:
    def __new__(cls, center: Vec3, radius: float) -> AnalyticSphere: ...
    center: Vec3
    radius: float
    def __repr__(self) -> str: ...

@final
class AnalyticTemplate:
    def __new__(cls, namespace: int, spheres: NDArray[np.float32], capsules: NDArray[np.float32]) -> AnalyticTemplate: ...
    capsules: list[AnalyticCapsule]
    part_count: int
    spheres: list[AnalyticSphere]

@final
class InstanceBatch:
    bounds: Aabb
    instance_count: int
    row_count: int
    style: InstanceStyle
    template: AnalyticTemplate
    visible: bool
    def copy_transforms_numpy(self) -> NDArray[np.float32]: ...
    def __repr__(self) -> str: ...

@final
class InstanceStyle:
    def __new__(cls, color: Rgba8 | None = None) -> InstanceStyle: ...
    color: Rgba8
    def __repr__(self) -> str: ...

@final
class RigidInstance:
    def __new__(cls, translation: Vec3, orientation: Quat, scale: float) -> RigidInstance: ...
    orientation: Quat
    scale: float
    translation: Vec3
    def __repr__(self) -> str: ...

@final
class Particle:
    def __new__(cls, owner: StructureHandle, center: Vec3, orientation: Quat, size: Vec3, shape: ParticleShape, color: Rgba8 | None = None, opacity: float = 1.0) -> Particle: ...
    bounds: Aabb
    center: Vec3
    color: Rgba8
    motion: ParticleMotion | None
    opacity: float
    orientation: Quat
    owner: StructureHandle
    shape: ParticleShape
    shape_parameters: list[float]
    size: Vec3
    visible: bool
    def with_motion(self, motion: ParticleMotion) -> Particle: ...
    def with_superquadric_exponents(self, latitude: float, longitude: float) -> Particle: ...
    def without_motion(self) -> Particle: ...
    def __repr__(self) -> str: ...

@final
class PointBatch:
    bounds: Aabb
    glyph: PointGlyph
    row_count: int
    style: PointStyle
    visible: bool
    def copy_positions_numpy(self) -> NDArray[np.float32]: ...
    def __repr__(self) -> str: ...

@final
class PointGlyph:
    Disc: PointGlyph
    Sphere: PointGlyph
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class PointStyle:
    def __new__(cls, radius: float = 0.1, color: Rgba8 | None = None) -> PointStyle: ...
    color: Rgba8
    radius: float
    def __repr__(self) -> str: ...

@final
class Primitive:
    bounds: Aabb
    carbohydrate: CarbohydrateSymbol | None
    ellipsoid: AnisotropicEllipsoid | None
    owner: StructureHandle
    particle: Particle | None
    planar: PlanarRegion | None
    visible: bool
    def __repr__(self) -> str: ...

@final
class StructureHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class RepresentationHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class SelectionHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class VolumeHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class SegmentationHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class EnsembleHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class AtomPropertyHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class MeshHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class MeshInstanceHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class OverlayHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class PrimitiveHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class AttributeHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class PointBatchHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class InstanceBatchHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class RelationBatchHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class TimelineTrackHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class GuideHandle:
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class InteractionHandle:
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class LigandPoseBatchHandle:
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class AnnotationHandle:
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class MeasurementHandle:
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class EntityKind:
    Atom: EntityKind
    Bond: EntityKind
    DynamicBond: EntityKind
    Edge: EntityKind
    Guide: EntityKind
    Instance: EntityKind
    Label: EntityKind
    LigandPoseBatch: EntityKind
    Mesh: EntityKind
    Point: EntityKind
    Primitive: EntityKind
    Relation: EntityKind
    TemplatePart: EntityKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class EntityRef:
    def __new__(cls, structure: StructureHandle, kind: EntityKind, index: int) -> EntityRef: ...
    index: int
    kind: EntityKind
    structure: StructureHandle

@final
class VolumeSegmentRef:
    def __new__(cls, volume: SegmentationHandle, label: int) -> VolumeSegmentRef: ...
    label: int
    volume: SegmentationHandle

@final
class InteractionAnchor:
    @staticmethod
    def entity(position: Vec3, entity: EntityRef) -> InteractionAnchor: ...
    @staticmethod
    def world(position: Vec3) -> InteractionAnchor: ...
    position: Vec3
    source_entity: EntityRef | None

@final
class InteractionGeometry:
    def __new__(cls, distance_angstrom: float, angle_degrees: float | None = None) -> InteractionGeometry: ...
    angle_degrees: float | None
    distance_angstrom: float

@final
class InteractionKind:
    Hydrophobic: InteractionKind
    HydrogenBond: InteractionKind
    MetalCoordination: InteractionKind
    PiStacking: InteractionKind
    SaltBridge: InteractionKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class InteractionDirection:
    Forward: InteractionDirection
    Reverse: InteractionDirection
    Undirected: InteractionDirection
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class InteractionPattern:
    Dashes: InteractionPattern
    Dots: InteractionPattern
    Solid: InteractionPattern
    Spring: InteractionPattern
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class InteractionStyle:
    color: Rgba8
    duty_cycle: float
    opacity: float
    pattern: InteractionPattern
    period_pixels: float
    phase_speed_pixels_per_frame: float
    width_pixels: float
    def __repr__(self) -> str: ...

@final
class InteractionEdge:
    def __new__(cls, owner: StructureHandle, start: InteractionAnchor, end: InteractionAnchor, kind: InteractionKind, geometry: InteractionGeometry, provenance: str) -> InteractionEdge: ...
    direction: InteractionDirection
    end: InteractionAnchor
    geometry: InteractionGeometry
    kind: InteractionKind
    normalized_strength: float | None
    occupancy: float | None
    owner: StructureHandle
    persistence_age_frames: int
    persistence_half_life_frames: float
    phase_speed_pixels_per_frame: float
    provenance: str
    start: InteractionAnchor
    visible: bool
    def resolved_style(self) -> InteractionStyle: ...
    def with_direction(self, direction: InteractionDirection) -> InteractionEdge: ...
    def with_normalized_strength(self, strength: float) -> InteractionEdge: ...
    def with_occupancy(self, occupancy: float) -> InteractionEdge: ...
    def with_persistence(self, age_frames: int, half_life_frames: float) -> InteractionEdge: ...
    def with_phase_speed(self, pixels_per_frame: float) -> InteractionEdge: ...
    def __repr__(self) -> str: ...

@final
class LicoriceTemplate:
    def __new__(cls, origin: Vec3, atoms: Sequence[Vec3], bonds: Sequence[tuple[int, int]]) -> LicoriceTemplate: ...
    atom_radius: float
    bond_radius: float
    instances_per_pose: int
    def with_radii(self, atom_radius: float, bond_radius: float) -> LicoriceTemplate: ...

@final
class LigandPose:
    def __new__(cls, translation: Vec3, orientation: Quat, color: Rgba8, opacity: float) -> LigandPose: ...
    color: Rgba8
    opacity: float
    orientation: Quat
    translation: Vec3

@final
class LigandPoseBatch:
    instance_count: int
    owner: StructureHandle
    pose_count: int
    poses: list[LigandPose]
    template: LicoriceTemplate
    visible: bool
    def __repr__(self) -> str: ...

@final
class Provenance:
    detail_kind: str
    entity: EntityRef
    entry_id: str | None
    entry_title: str | None
    method: str | None
    resolution: float | None

@final
class RepresentationKind:
    BallAndStick: RepresentationKind
    Beads: RepresentationKind
    Cartoon: RepresentationKind
    Licorice: RepresentationKind
    Lines: RepresentationKind
    PaperChain: RepresentationKind
    Points: RepresentationKind
    Rocket: RepresentationKind
    Segmentation: RepresentationKind
    Spacefill: RepresentationKind
    Surface: RepresentationKind
    Trace: RepresentationKind
    Tube: RepresentationKind
    Twister: RepresentationKind
    Volume: RepresentationKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class PropertyAppearance:
    def __new__(cls, property: AtomPropertyHandle, domain: tuple[float, float], opacity: tuple[float, float], softness_pixels: tuple[float, float], missing: PropertyAppearanceSample) -> PropertyAppearance: ...
    @staticmethod
    def confidence(property: AtomPropertyHandle, domain: tuple[float, float]) -> PropertyAppearance: ...
    @staticmethod
    def flexibility(property: AtomPropertyHandle, domain: tuple[float, float]) -> PropertyAppearance: ...
    domain: tuple[float, float]
    is_translucent: bool
    missing: PropertyAppearanceSample
    opacity: tuple[float, float]
    property: AtomPropertyHandle
    softness_pixels: tuple[float, float]
    def sample(self, value_: float) -> PropertyAppearanceSample: ...
    def value_from_opacity(self, opacity: float) -> float | None: ...
    def value_from_softness(self, softness_pixels: float) -> float | None: ...
    def __repr__(self) -> str: ...

@final
class PropertyAppearanceSample:
    def __new__(cls, opacity: float, softness_pixels: float) -> PropertyAppearanceSample: ...
    opacity: float
    softness_pixels: float
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Representation:
    kind: RepresentationKind
    @staticmethod
    def ball_and_stick() -> Representation: ...
    @staticmethod
    def beads() -> Representation: ...
    @staticmethod
    def cartoon() -> Representation: ...
    @staticmethod
    def licorice() -> Representation: ...
    @staticmethod
    def lines() -> Representation: ...
    @staticmethod
    def paper_chain() -> Representation: ...
    @staticmethod
    def points() -> Representation: ...
    @staticmethod
    def rocket() -> Representation: ...
    @staticmethod
    def segmentation() -> Representation: ...
    @staticmethod
    def spacefill() -> Representation: ...
    @staticmethod
    def surface() -> Representation: ...
    @staticmethod
    def trace() -> Representation: ...
    @staticmethod
    def tube() -> Representation: ...
    @staticmethod
    def twister() -> Representation: ...
    @staticmethod
    def volume() -> Representation: ...
    def appearance(self, appearance: PropertyAppearance) -> Representation: ...
    def bond_radius(self, radius: float) -> Representation: ...
    def clipping(self, clipping: ClipSet) -> Representation: ...
    def color(self, color: ColorScheme) -> Representation: ...
    def isolevel(self, level: float) -> Representation: ...
    def material(self, material: Material) -> Representation: ...
    def radius_scale(self, scale: float) -> Representation: ...
    def segmentation_style(self, style: SegmentationStyle) -> Representation: ...
    def surface_presentation(self, kind: SurfaceKind, style: SurfaceStyle) -> Representation: ...
    def surface_scalar(self, overlay: SurfaceScalarOverlay) -> Representation: ...
    def visual(self, style: VisualStyle) -> Representation: ...
    def volume_style(self, style: VolumeStyle) -> Representation: ...
    def __repr__(self) -> str: ...

@final
class RepresentationPreset:
    @staticmethod
    def cpk() -> RepresentationPreset: ...
    @staticmethod
    def dotted_solvent() -> RepresentationPreset: ...
    @staticmethod
    def licorice() -> RepresentationPreset: ...
    @staticmethod
    def paper_chain() -> RepresentationPreset: ...
    @staticmethod
    def signed_isosurface(negative_level: float, positive_level: float, negative_color: Rgba8, positive_color: Rgba8) -> RepresentationPreset: ...

@final
class RepresentationTarget:
    segmentation_handle: SegmentationHandle | None
    selection_handle: SelectionHandle | None
    volume_handle: VolumeHandle | None
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class RelationStyle:
    def __new__(cls, width_pixels: float = 1.5, color: Rgba8 | None = None, opacity: float = 1.0, pattern: RelationPattern | None = None, endpoint_insets_pixels: tuple[float, float] = ..., depth_behind_anchors: bool = False) -> RelationStyle: ...
    @staticmethod
    def default() -> RelationStyle: ...
    width_pixels: float
    color: Rgba8
    opacity: float
    pattern: RelationPattern
    endpoint_insets_pixels: tuple[float, float]
    depth_behind_anchors: bool

@final
class Ensemble:
    def __new__(cls, members: Sequence[StructureHandle], weights: Sequence[float], provenance: str) -> Ensemble: ...
    members: list[StructureHandle]
    weights: list[float]
    provenance: str
    dominant_index: int
    def __repr__(self) -> str: ...

@final
class Scene:
    def __new__(cls) -> Scene: ...
    interaction_count: int
    interaction_revision: int
    presentation_revision: int
    presentation_time_seconds: float
    representation_count: int
    representation_revision: int
    structure_count: int
    def add_annotation(self, annotation: Annotation) -> AnnotationHandle: ...
    def add_atom_property(self, property: AtomProperty) -> AtomPropertyHandle: ...
    def add_category_attribute_from_numpy(self, domain: RowDomain, name: str, values: NDArray[np.uint32]) -> AttributeHandle: ...
    def add_color_attribute_from_numpy(self, domain: RowDomain, name: str, values: NDArray[np.uint8]) -> AttributeHandle: ...
    def add_color_legend(self, title: str, range: tuple[float, float], colors: tuple[Rgba8, Rgba8], size_pixels: tuple[float, float], anchor: OverlayAnchor, order: int = 0) -> OverlayHandle: ...
    def add_coordinate_tripod(self, anchor: OverlayAnchor, size_pixels: float = 48.0, width_pixels: float = 2.0, order: int = 0) -> OverlayHandle: ...
    def add_ensemble(self, ensemble: Ensemble) -> EnsembleHandle: ...
    def add_entity_relations_from_numpy(self, namespace: int, start_domain: RowDomain, start_rows: NDArray[np.uint32], end_domain: RowDomain, end_rows: NDArray[np.uint32], style: RelationStyle | None = None) -> RelationBatchHandle: ...
    def add_instance_batch_from_numpy(self, template: AnalyticTemplate, namespace: int, translations: NDArray[np.float32], orientations: NDArray[np.float32], scales: NDArray[np.float32], style: InstanceStyle | None = None) -> InstanceBatchHandle: ...
    def add_interaction(self, interaction: InteractionEdge) -> InteractionHandle: ...
    def add_licorice_poses(self, owner: StructureHandle, template: LicoriceTemplate, poses: Sequence[LigandPose]) -> LigandPoseBatchHandle | None: ...
    def add_licorice_poses_from_numpy(self, owner: StructureHandle, template: LicoriceTemplate, translations: NDArray[np.float32], orientations: NDArray[np.float32], colors: NDArray[np.uint8], opacities: NDArray[np.float32]) -> LigandPoseBatchHandle | None: ...
    def add_measurement(self, measurement: Measurement) -> MeasurementHandle: ...
    def add_mesh_instance(self, mesh: MeshHandle, transform: Mat4) -> MeshInstanceHandle: ...
    def add_occupancy_stream(self, structure: StructureHandle, selection: Select | SelectionHandle | str, stream: OccupancyStream) -> VolumeHandle: ...
    def add_point_batch_from_numpy(self, namespace: int, positions: NDArray[np.float32], style: PointStyle | None = None, glyph: PointGlyph = PointGlyph.Disc) -> PointBatchHandle: ...
    def add_scalar_attribute_from_numpy(self, domain: RowDomain, name: str, values: NDArray[np.float32]) -> AttributeHandle: ...
    def add_scale_bar(self, length_angstrom: float, anchor: OverlayAnchor, color: Rgba8, width_pixels: float = 2.0, order: int = 0) -> OverlayHandle: ...
    def add_segmented_volume(self, volume: SegmentedVolume) -> SegmentationHandle: ...
    def add_structure_shared(self, object: object) -> StructureHandle: ...
    def add_text_overlay(self, text: str, anchor: OverlayAnchor, color: Rgba8, size_pixels: float = 14.0, order: int = 0) -> OverlayHandle: ...
    def add_unit_cell(self, owner: StructureHandle, cell: CrystalCell, style: GuideStyle) -> list[GuideHandle]: ...
    def add_validation_marker(self, marker: ValidationMarker) -> AnnotationHandle: ...
    def add_vector_attribute_from_numpy(self, domain: RowDomain, name: str, values: NDArray[np.float32]) -> AttributeHandle: ...
    def add_volume(self, volume: ScalarVolume) -> VolumeHandle: ...
    def add_world_relations_from_numpy(self, namespace: int, starts: NDArray[np.float32], ends: NDArray[np.float32], style: RelationStyle | None = None) -> RelationBatchHandle: ...
    def annotation(self, handle: AnnotationHandle) -> Annotation | None: ...
    def attribute(self, handle: AttributeHandle) -> AttributeColumn | None: ...
    def attribute_change(self, handle: AttributeHandle) -> tuple[int, int, int] | None: ...
    attribute_revision: int
    def attributes(self) -> list[tuple[AttributeHandle, AttributeColumn]]: ...
    def clear_bond_topology(self, structure: StructureHandle) -> bool: ...
    def clear_trajectory(self, structure: StructureHandle) -> bool: ...
    def compose_difference(self, domains: Sequence[RowDomain], deltas: Sequence[AttributeHandle], context_threshold: float, emphasis_threshold: float, ramp: ScalarRamp, context_opacity: float = 0.16, order: int = 0) -> GenericCompositionView: ...
    def compose_ensemble(self, domains: Sequence[RowDomain], weights: Sequence[float], colors: Sequence[Rgba8], dominant_opacity: float = 1.0, alternate_opacity: float = 0.55, minimum_opacity: float = 0.08, order: int = 0) -> GenericCompositionView: ...
    def compose_focus(self, domain: RowDomain, emphasis: AttributeHandle, context_opacity: float = 0.16, focus_opacity: float = 1.0, order: int = 0) -> GenericCompositionView: ...
    def copy_manifest_json(self) -> bytes: ...
    def copy_mesh_from_numpy(self, owner: StructureHandle, positions: NDArray[np.float32], normals: NDArray[np.float32], colors: NDArray[np.uint8], indices: NDArray[np.uint32], material: Material) -> MeshHandle: ...
    def copy_mesh_instances_from_numpy(self, mesh: MeshHandle, transforms: NDArray[np.float32]) -> MeshInstanceHandle | None: ...
    def copy_particles_from_numpy(self, owner: StructureHandle, centers: NDArray[np.float32], sizes: NDArray[np.float32], orientations: NDArray[np.float32], colors: NDArray[np.uint8], opacities: NDArray[np.float32], shape: ParticleShape) -> PrimitiveHandle | None: ...
    def copy_polyline_from_numpy(self, owner: StructureHandle, points: NDArray[np.float32], closed: bool, style: GuideStyle) -> list[GuideHandle]: ...
    def describe(self) -> SceneDescription: ...
    def domain_row_count(self, domain: RowDomain) -> int | None: ...
    def domain_visual(self, domain: RowDomain) -> VisualDescriptor | None: ...
    def domain_visuals(self) -> list[tuple[RowDomain, VisualDescriptor]]: ...
    def ensemble(self, handle: EnsembleHandle) -> Ensemble | None: ...
    def ensembles(self) -> list[tuple[EnsembleHandle, Ensemble]]: ...
    @staticmethod
    def from_manifest_json(source: bytes, structures: Sequence[object], volumes: Sequence[ScalarVolume] | None = None, segmentations: Sequence[SegmentedVolume] | None = None) -> Scene: ...
    @staticmethod
    def from_structure_shared(object: object) -> Scene: ...
    def hide(self, representation: RepresentationHandle) -> None: ...
    def hide_ligand_pose_batch(self, handle: LigandPoseBatchHandle) -> bool: ...
    def interaction(self, handle: InteractionHandle) -> InteractionEdge | None: ...
    def interaction_for_entity(self, entity: EntityRef) -> tuple[InteractionHandle, InteractionEdge] | None: ...
    def interactions(self) -> list[tuple[InteractionHandle, InteractionEdge]]: ...
    def ligand_pose_batch(self, handle: LigandPoseBatchHandle) -> LigandPoseBatch | None: ...
    def ligand_pose_batch_for_entity(self, entity: EntityRef) -> LigandPoseBatch | None: ...
    def ligand_pose_batches(self) -> list[tuple[LigandPoseBatchHandle, LigandPoseBatch]]: ...
    def interpolate_atom_property_shared(self, property: AtomPropertyHandle, start: NDArray[np.float32], end: NDArray[np.float32], alpha: float) -> None: ...
    def manifest(self, payloads: Sequence[PayloadReference]) -> SceneManifest: ...
    def measurement(self, handle: MeasurementHandle) -> Measurement | None: ...
    def mesh(self, handle: MeshHandle) -> Mesh | None: ...
    def mesh_instance(self, handle: MeshInstanceHandle) -> MeshInstance | None: ...
    def mesh_instances(self) -> list[tuple[MeshInstanceHandle, MeshInstance]]: ...
    def meshes(self) -> list[tuple[MeshHandle, Mesh]]: ...
    def provenance(self, entity: EntityRef) -> Provenance | None: ...
    def remove_attribute(self, handle: AttributeHandle) -> AttributeColumn | None: ...
    def remove_domain_visual(self, domain: RowDomain) -> bool: ...
    def remove_ensemble(self, handle: EnsembleHandle) -> Ensemble | None: ...
    def remove_interaction(self, handle: InteractionHandle) -> InteractionEdge | None: ...
    def remove_ligand_pose_batch(self, handle: LigandPoseBatchHandle) -> LigandPoseBatch | None: ...
    def instance_batch(self, handle: InstanceBatchHandle) -> InstanceBatch | None: ...
    def instance_batches(self) -> list[tuple[InstanceBatchHandle, InstanceBatch]]: ...
    def overlay(self, handle: OverlayHandle) -> ScreenOverlay | None: ...
    def overlays(self) -> list[tuple[OverlayHandle, ScreenOverlay]]: ...
    def point_batch(self, handle: PointBatchHandle) -> PointBatch | None: ...
    def point_batches(self) -> list[tuple[PointBatchHandle, PointBatch]]: ...
    def primitive(self, handle: PrimitiveHandle) -> Primitive | None: ...
    def primitive_for_entity(self, entity: EntityRef) -> Primitive | None: ...
    def primitives(self) -> list[tuple[PrimitiveHandle, Primitive]]: ...
    def remove_instance_batch(self, handle: InstanceBatchHandle) -> InstanceBatch | None: ...
    def remove_mesh_instance(self, handle: MeshInstanceHandle) -> bool: ...
    def remove_point_batch(self, handle: PointBatchHandle) -> PointBatch | None: ...
    def remove_primitive(self, handle: PrimitiveHandle) -> Primitive | None: ...
    def remove_relation_batch(self, handle: RelationBatchHandle) -> RelationBatch | None: ...
    def relation_batch(self, handle: RelationBatchHandle) -> RelationBatch | None: ...
    def relation_batches(self) -> list[tuple[RelationBatchHandle, RelationBatch]]: ...
    def remove_overlay(self, handle: OverlayHandle) -> bool: ...
    def remove_representation(self, representation: RepresentationHandle) -> None: ...
    def replace_occupancy_stream(self, handle: VolumeHandle, structure: StructureHandle, selection: Select | SelectionHandle | str, stream: OccupancyStream) -> None: ...
    def represent(self, target: Select | SelectionHandle | str | VolumeHandle | SegmentationHandle, representation: Representation) -> RepresentationHandle: ...
    def represent_preset(self, selection: Select | SelectionHandle | str, preset: RepresentationPreset) -> list[RepresentationHandle]: ...
    def representation_kind(self, representation: RepresentationHandle) -> RepresentationKind | None: ...
    def representation_target(self, representation: RepresentationHandle) -> RepresentationTarget | None: ...
    def select(self, query: Select) -> SelectionHandle: ...
    def select_str(self, source: str) -> SelectionHandle: ...
    def set_bond_topology_segment(self, structure: StructureHandle, segment: BondTopologySegment) -> None: ...
    def set_bond_topology_time(self, structure: StructureHandle, seconds: float) -> None: ...
    def set_domain_visible(self, domain: RowDomain, visible: bool) -> bool: ...
    def set_domain_visual(self, domain: RowDomain, style: VisualStyle, order: int = 0) -> None: ...
    def set_interaction_visible(self, handle: InteractionHandle, visible: bool) -> bool: ...
    def set_mesh_instance_transform(self, handle: MeshInstanceHandle, transform: Mat4) -> bool: ...
    def set_mesh_instance_visible(self, handle: MeshInstanceHandle, visible: bool) -> bool: ...
    def set_overlay_visible(self, handle: OverlayHandle, visible: bool) -> bool: ...
    def set_presentation_time(self, seconds: float) -> None: ...
    def set_representation_visual(self, representation: RepresentationHandle, style: VisualStyle | None) -> None: ...
    def set_trajectory_segment(self, structure: StructureHandle, segment: TrajectorySegment) -> None: ...
    def set_trajectory_time(self, structure: StructureHandle, seconds: float) -> None: ...
    def set_visual_color_parameter(self, representation: RepresentationHandle, index: int, value_: tuple[float, float, float, float]) -> None: ...
    def set_visual_scalar_parameter(self, representation: RepresentationHandle, index: int, value_: float) -> None: ...
    def set_visual_vector_parameter(self, representation: RepresentationHandle, index: int, value_: tuple[float, float, float]) -> None: ...
    def show(self, representation: RepresentationHandle) -> None: ...
    def show_ligand_pose_batch(self, handle: LigandPoseBatchHandle) -> bool: ...
    def world_aabb(self) -> Aabb: ...
    def __repr__(self) -> str: ...

@final
class PropertyComparison:
    Equal: PropertyComparison
    Greater: PropertyComparison
    GreaterOrEqual: PropertyComparison
    Less: PropertyComparison
    LessOrEqual: PropertyComparison
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class SecondaryStructure:
    Coil: SecondaryStructure
    Helix: SecondaryStructure
    Strand: SecondaryStructure
    Turn: SecondaryStructure
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class Select:
    @staticmethod
    def all() -> Select: ...
    def and_(self, other: Select) -> Select: ...
    @staticmethod
    def atom_name(name: str) -> Select: ...
    @staticmethod
    def b_factor(comparison: PropertyComparison, threshold: float) -> Select: ...
    @staticmethod
    def backbone() -> Select: ...
    @staticmethod
    def beyond(distance: float, reference: Select) -> Select: ...
    @staticmethod
    def branched() -> Select: ...
    @staticmethod
    def chain(label: str) -> Select: ...
    @staticmethod
    def coil() -> Select: ...
    @staticmethod
    def element(symbol: str) -> Select: ...
    @staticmethod
    def heavy() -> Select: ...
    @staticmethod
    def helix() -> Select: ...
    @staticmethod
    def hydrogen() -> Select: ...
    @staticmethod
    def in_box(min: Vec3, max: Vec3) -> Select: ...
    @staticmethod
    def in_sphere(center: Vec3, radius: float) -> Select: ...
    @staticmethod
    def ligands() -> Select: ...
    def negate(self) -> Select: ...
    @staticmethod
    def none() -> Select: ...
    @staticmethod
    def nucleic() -> Select: ...
    @staticmethod
    def occupancy(comparison: PropertyComparison, threshold: float) -> Select: ...
    def or_(self, other: Select) -> Select: ...
    @staticmethod
    def parse(source: str) -> Select: ...
    @staticmethod
    def polymer() -> Select: ...
    @staticmethod
    def protein() -> Select: ...
    @staticmethod
    def residue(number: int) -> Select: ...
    @staticmethod
    def residue_name(name: str) -> Select: ...
    @staticmethod
    def residues_within(distance: float, reference: Select) -> Select: ...
    @staticmethod
    def secondary(value: SecondaryStructure) -> Select: ...
    @staticmethod
    def sheet() -> Select: ...
    @staticmethod
    def terminus() -> Select: ...
    @staticmethod
    def water() -> Select: ...
    @staticmethod
    def within(distance: float, reference: Select) -> Select: ...
    def __repr__(self) -> str: ...

@final
class ScalarVolume:
    def __new__(cls, dimensions: tuple[int, int, int], values: NDArray[np.float32], voxel_to_world: Mat4 | None = None) -> ScalarVolume: ...
    dimensions: tuple[int, int, int]
    range: tuple[float, float]
    values: NDArray[np.float32]
    voxel_to_world: Mat4

@final
class OccupancyStream:
    def __new__(cls, dimensions: tuple[int, int, int], origin: tuple[float, float, float], spacing: tuple[float, float, float], decay: float, deposit: float, maximum: float) -> OccupancyStream: ...
    decay: float
    deposit: float
    dimensions: tuple[int, int, int]
    maximum: float
    voxel_to_model: Mat4

@final
class SegmentedVolume:
    def __new__(cls, dimensions: tuple[int, int, int], labels: NDArray[np.uint32], voxel_to_world: Mat4 | None = None) -> SegmentedVolume: ...
    dimensions: tuple[int, int, int]
    labels: NDArray[np.uint32]
    voxel_to_world: Mat4

@final
class MaterialModel:
    @staticmethod
    def anisotropic_ribbon(strength: float) -> MaterialModel: ...
    @staticmethod
    def diffusion(strength: float) -> MaterialModel: ...
    @staticmethod
    def molecular() -> MaterialModel: ...
    @staticmethod
    def principled(metallic: float) -> MaterialModel: ...
    def __repr__(self) -> str: ...

@final
class Material:
    model: MaterialModel
    opacity: float
    roughness: float
    specular: float
    @staticmethod
    def anisotropic_ribbon(strength: float) -> Material: ...
    @staticmethod
    def default() -> Material: ...
    @staticmethod
    def diffusion(strength: float) -> Material: ...
    @staticmethod
    def principled(metallic: float) -> Material: ...

@final
class MeshDescription:
    content_hash: int
    face_visibility: str
    generation: int
    row: int
    visible: bool

@final
class MeshInstanceDescription:
    generation: int
    row: int
    transform: list[float]
    visible: bool

@final
class OverlayDescription:
    generation: int
    kind: str
    row: int
    visible: bool
    @staticmethod
    def copy_from_values(row: int, generation: int, kind: str, normalized: tuple[float, float], pixels: tuple[float, float], order: int, text: str, colors: tuple[Rgba8, Rgba8], values: tuple[float, float, float, float], visible: bool = True) -> OverlayDescription: ...

@final
class ParticleMotionDescription:
    def __new__(cls, motion: ParticleMotion) -> ParticleMotionDescription: ...
    boundary: ParticleBoundary

@final
class PrimitiveDescription:
    center: list[float]
    color: list[int]
    generation: int
    kind: str
    row: int
    visible: bool

@final
class ObjectIdentity:
    def __new__(cls, row: int, generation: int) -> ObjectIdentity: ...
    generation: int
    row: int
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class EntityDescription:
    index: int
    kind: str
    structure: ObjectIdentity

@final
class AnchorDescription:
    entity: EntityDescription | None
    position: list[float]

@final
class SelectionMask:
    atoms: list[int]
    structure_row: int

@final
class TargetDescription:
    def __new__(cls, kind: str, row: int, generation: int) -> TargetDescription: ...
    generation: int
    kind: str
    row: int

@final
class RowDomainDescription:
    generation: int
    kind: str
    row: int

@final
class SourceRowsDescription:
    keyed: bool
    namespace: int
    row_count: int

@final
class SceneDescription:
    annotations: list[AnnotationDescription]
    atom_properties: list[AtomPropertyDescription]
    attributes: list[AttributeDescription]
    domain_visuals: list[DomainVisualDescription]
    engine: str
    guides: list[GuideDescription]
    instance_batches: list[InstanceBatchDescription]
    interactions: list[InteractionDescription]
    ligand_pose_batches: list[LigandPoseBatchDescription]
    measurements: list[MeasurementDescription]
    mesh_instances: list[MeshInstanceDescription]
    meshes: list[MeshDescription]
    overlays: list[OverlayDescription]
    point_batches: list[PointBatchDescription]
    primitives: list[PrimitiveDescription]
    relation_batches: list[RelationBatchDescription]
    representations: list[RepresentationDescription]
    schema: int
    segmentations: list[VolumeDescription]
    selections: list[SelectionDescription]
    structures: list[StructureDescription]
    tables: TableCounts
    volumes: list[VolumeDescription]

@final
class StructureDescription:
    atom_count: int
    coordinate_hash: int
    dataset_id: int
    generation: int
    method: str | None
    model_to_world: list[float]
    resolution: float | None
    row: int
    secondary_structure: list[str]
    source_id: str | None
    title: str | None

@final
class SelectionDescription:
    generation: int
    masks: list[SelectionMask]
    row: int

@final
class TableCounts:
    annotations: int
    atom_properties: int
    attributes: int
    domain_visuals: int
    guides: int
    instance_batches: int
    interactions: int
    ligand_pose_batches: int
    measurements: int
    mesh_instances: int
    overlays: int
    point_batches: int
    relation_batches: int

@final
class RepresentationDescription:
    appearance: PropertyAppearanceDescription | None
    clipping: ClipDescription
    color: ColorDescription
    generation: int
    kind: str
    material: MaterialDescription
    order: int
    params: list[float]
    row: int
    segmentation: SegmentationStyleDescription
    surface_components: SurfaceComponentDescription
    surface_scalar: SurfaceScalarDescription | None
    target: TargetDescription
    tube_radius_mapping: list[float] | None
    visible: bool
    visual: VisualStyleDescription | None
    volume: VolumeStyleDescription

@final
class SurfaceComponentDescription:
    Disabled: SurfaceComponentDescription
    measure: str
    minimum_area: float | None
    minimum_volume: float | None
    minimum_voxels: int | None
    @staticmethod
    def area(square_angstrom: float) -> SurfaceComponentDescription: ...
    @staticmethod
    def volume(cubic_angstrom: float) -> SurfaceComponentDescription: ...
    @staticmethod
    def voxels(count: int) -> SurfaceComponentDescription: ...
    def __repr__(self) -> str: ...

@final
class VisualStyleDescription:
    attributes: list[VisualAttributeDescription]
    instructions: list[VisualInstructionDescription]
    maximum_displacement: float
    outputs: list[list[int]]
    parameter_defaults: list[list[float]]
    parameter_kinds: list[int]
    parameters: list[list[float]]
    properties: list[ObjectIdentity]

@final
class VisualAttributeDescription:
    identity: ObjectIdentity
    kind: str

@final
class VisualInstructionDescription:
    data: list[float]
    kind: int
    opcode: int
    operands: list[int]
    stage: int

@final
class ColorDescription:
    mode: str
    property_generation: int | None
    property_row: int | None
    ramp_colors: list[list[int]] | None
    ramp_values: list[int] | None
    rgba: list[int] | None

@final
class MaterialDescription:
    model: str
    model_parameter: float
    response: list[float]

@final
class ClipDescription:
    cap: str
    planes: list[list[float]]

@final
class VolumeDescription:
    content_hash: int
    dimensions: list[int]
    generation: int
    occupancy: OccupancyDescription | None
    range: list[float]
    row: int
    voxel_to_world: list[float]

@final
class OccupancyDescription:
    atom_rows: list[int]
    decay: float
    deposit: float
    maximum: float
    structure: ObjectIdentity
    voxel_to_model: list[float]

@final
class VolumeStyleDescription:
    opacity_scale: float
    region: RegionDescription | None
    rendering: str
    slice: list[float] | None
    step_scale: float
    transfer: list[VolumeTransferPointDescription]

@final
class VolumeTransferPointDescription:
    color: list[int]
    opacity: float
    value: float

@final
class SegmentationStyleDescription:
    opacity_scale: float
    region: RegionDescription | None
    slice: list[float] | None
    step_scale: float
    styles: list[SegmentStyleDescription]

@final
class SegmentStyleDescription:
    color: list[int]
    label: int
    opacity: float

@final
class RegionDescription:
    maximum: list[int]
    minimum: list[int]

@final
class ScalarSemanticsDescription:
    kind: str
    name: str | None
    provenance: str | None
    units: str | None

@final
class AtomPropertyDescription:
    content_hash: int
    finite_domain: list[float]
    generation: int
    length: int
    meaning: str
    name: str
    owner: ObjectIdentity
    row: int
    semantics: ScalarSemanticsDescription

@final
class PropertyAppearanceDescription:
    domain: list[float]
    missing: list[float]
    opacity: list[float]
    property: ObjectIdentity
    softness_pixels: list[float]

@final
class SurfaceScalarDescription:
    contours: list[float] | None
    field: ObjectIdentity
    ramp_colors: list[list[int]]
    ramp_values: list[int]
    sample_offset_angstrom: float

@final
class PointBatchDescription:
    color: list[int]
    generation: int
    glyph: str
    payload: PayloadReference
    radius_bits: int
    row: int
    source_rows: SourceRowsDescription
    visible: bool

@final
class InstanceBatchDescription:
    capsule_count: int
    color: list[int]
    generation: int
    payload: PayloadReference
    row: int
    source_rows: SourceRowsDescription
    sphere_count: int
    template_rows: SourceRowsDescription
    visible: bool

@final
class AttributeDescription:
    domain: RowDomainDescription
    fingerprint: int
    generation: int
    kind: str
    name: str
    payload: PayloadReference
    provenance: str | None
    quantity: str | None
    row: int
    row_count: int
    unit: str | None

@final
class RelationBatchDescription:
    color: list[int]
    depth_behind_anchors: bool
    endpoint_inset_bits: list[int]
    generation: int
    opacity_bits: int
    pattern: str
    payload: PayloadReference
    row: int
    source_rows: SourceRowsDescription
    visible: bool
    width_bits: int

@final
class DomainVisualDescription:
    domain: RowDomainDescription
    order: int
    style: VisualStyleDescription

@final
class GuideStyleDescription:
    arrow_pixels: float
    cap: str
    color: list[int]
    duty_cycle: float
    opacity: float
    pattern: str
    period_pixels: float
    width_pixels: float

@final
class GuideDescription:
    end: list[float]
    generation: int
    owner: ObjectIdentity
    row: int
    start: list[float]
    style: GuideStyleDescription
    visible: bool

@final
class MarkerStyleDescription:
    color: list[int]
    radius_pixels: float
    shape: str

@final
class AnnotationDescription:
    anchor: AnchorDescription | None
    generation: int
    kind: str
    marker: MarkerStyleDescription
    owner: ObjectIdentity
    priority: int
    region: ObjectIdentity | None
    row: int
    text: str
    visible: bool

@final
class MeasurementDescription:
    anchors: list[AnchorDescription]
    generation: int
    kind: str
    label: str
    owner: ObjectIdentity
    priority: int
    provenance: str
    row: int
    value: float
    visible: bool

@final
class InteractionDescription:
    angle_degrees: float | None
    direction: str
    distance_angstrom: float
    end: AnchorDescription
    generation: int
    kind: str
    normalized_strength: float | None
    occupancy: float | None
    owner: ObjectIdentity
    persistence_age_frames: int
    persistence_half_life_frames: float
    phase_speed_pixels_per_frame: float
    provenance: str
    row: int
    start: AnchorDescription
    visible: bool

@final
class LigandPoseDescription:
    color: list[int]
    opacity: float
    orientation: list[float]
    translation: list[float]

@final
class LigandPoseBatchDescription:
    atom_radius: float
    atoms: list[list[float]]
    bond_radius: float
    bonds: list[list[int]]
    generation: int
    owner: ObjectIdentity
    poses: list[LigandPoseDescription]
    row: int
    visible: bool

@final
class ContentAddress:
    hex: str
    @staticmethod
    def digest(payload: bytes) -> ContentAddress: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class ReferencedPayloadKind:
    Attribute: ReferencedPayloadKind
    Brick: ReferencedPayloadKind
    Frames: ReferencedPayloadKind
    Instances: ReferencedPayloadKind
    Mesh: ReferencedPayloadKind
    Points: ReferencedPayloadKind
    Property: ReferencedPayloadKind
    Proxy: ReferencedPayloadKind
    Relations: ReferencedPayloadKind
    Structure: ReferencedPayloadKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class PayloadReference:
    address: ContentAddress
    byte_len: int
    chunk: int
    dataset: int
    kind: ReferencedPayloadKind
    @staticmethod
    def of(kind: ReferencedPayloadKind, dataset: int, chunk: int, payload: bytes) -> PayloadReference: ...
    def __repr__(self) -> str: ...

@final
class SceneManifest:
    merkle_root: ContentAddress
    payloads: list[PayloadReference]
    scene: SceneDescription
    def copy_json(self) -> bytes: ...
    def __repr__(self) -> str: ...


def read_manifest(path: str | os.PathLike[str]) -> SceneManifest:
    """Reads and validates a manifest from a file."""


def write_manifest(manifest: SceneManifest, path: str | os.PathLike[str]) -> None:
    """Writes a validated manifest to a file."""


@final
class AnisotropicEllipsoid:
    def __new__(cls, center: Vec3, tensor: tuple[float, ...]) -> AnisotropicEllipsoid: ...
    bounds: Aabb
    center: Vec3
    inverse_tensor: list[float] | None
    tensor: list[float]

@final
class CarbohydrateShape:
    Fuc: CarbohydrateShape
    Gal: CarbohydrateShape
    Glc: CarbohydrateShape
    Man: CarbohydrateShape
    Neu5Ac: CarbohydrateShape
    Unknown: CarbohydrateShape
    Xyl: CarbohydrateShape
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class CarbohydrateSymbol:
    def __new__(cls, owner: StructureHandle, center: Vec3, orientation: Quat, size: Vec3, shape: CarbohydrateShape, color: Rgba8) -> CarbohydrateSymbol: ...
    bounds: Aabb
    center: Vec3
    color: Rgba8
    orientation: Quat
    owner: StructureHandle
    shape: CarbohydrateShape
    size: Vec3
    visible: bool

@final
class Guide:
    def __new__(cls, owner: StructureHandle, start: Vec3, end: Vec3, style: GuideStyle) -> Guide: ...
    end: Vec3
    owner: StructureHandle
    start: Vec3
    visible: bool
    def set_visible(self, visible: bool) -> None: ...
    def set_style(self, style: GuideStyle) -> None: ...

@final
class PlanarRegion:
    def __new__(cls, owner: StructureHandle, center: Vec3, normal: Vec3, tangent: Vec3, size: tuple[float, float]) -> PlanarRegion: ...
    bitangent: Vec3
    center: Vec3
    normal: Vec3
    owner: StructureHandle
    size: list[float]
    tangent: Vec3

@final
class Quadric:
    def __new__(cls, coefficients: tuple[float, ...], bounds: Aabb, cells: tuple[int, int, int]) -> Quadric: ...
    bounds: Aabb
    coefficients: list[float]

@final
class OverlayContent:
    color: Rgba8 | None
    colors: tuple[Rgba8, Rgba8] | None
    kind: OverlayKind
    length_angstrom: float | None
    range: tuple[float, float] | None
    size_pixels: tuple[float, float] | None
    text: str | None
    title: str | None
    width_pixels: float | None
    def __repr__(self) -> str: ...

@final
class OverlayKind:
    ColorLegend: OverlayKind
    CoordinateTripod: OverlayKind
    ScaleBar: OverlayKind
    Text: OverlayKind
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ScreenOverlay:
    anchor: OverlayAnchor
    content: OverlayContent
    order: int
    visible: bool
    @staticmethod
    def text(text: str, anchor: OverlayAnchor, color: Rgba8, size_pixels: float = 14.0) -> ScreenOverlay: ...
    @staticmethod
    def coordinate_tripod(anchor: OverlayAnchor, size_pixels: float, width_pixels: float) -> ScreenOverlay: ...
    def set_anchor(self, anchor: OverlayAnchor) -> None: ...
    def set_order(self, order: int) -> None: ...
    def set_visible(self, visible: bool) -> None: ...

@final
class ParticleBoundary:
    Bounce: ParticleBoundary
    Wrap: ParticleBoundary
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ParticleMotion:
    def __new__(cls, velocity: Vec3, bounds: Aabb, fixed_timestep: float, seed: int, boundary: ParticleBoundary = ..., respawn_after_steps: int = 0) -> ParticleMotion: ...
    boundary: ParticleBoundary
    bounds: Aabb
    fixed_timestep: float
    respawn_after_steps: int
    seed: int
    velocity: Vec3

@final
class FaceVisibility:
    BackOnly: FaceVisibility
    DoubleSided: FaceVisibility
    FrontOnly: FaceVisibility
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class PropertyLegend:
    def __new__(cls, title: str, semantics: ScalarFieldSemantics, values: tuple[float, float, float], colors: tuple[Rgba8, Rgba8, Rgba8], missing: Rgba8) -> PropertyLegend: ...
    colors: list[Rgba8]
    missing: Rgba8
    semantics: ScalarFieldSemantics
    title: str
    values: list[float]

@final
class SurfaceComponentPolicy:
    def __new__(cls) -> SurfaceComponentPolicy: ...
    maximum_components: int | None
    threshold: SurfaceComponentThreshold
    @staticmethod
    def keep_all() -> SurfaceComponentPolicy: ...
    @staticmethod
    def minimum_area(area: float) -> SurfaceComponentPolicy: ...
    @staticmethod
    def minimum_volume(volume: float) -> SurfaceComponentPolicy: ...
    @staticmethod
    def minimum_voxels(voxels: int) -> SurfaceComponentPolicy: ...
    def with_maximum_components(self, count: int) -> SurfaceComponentPolicy: ...
    def is_enabled(self) -> bool: ...

@final
class SurfaceComponentThreshold:
    Disabled: SurfaceComponentThreshold
    measure: str
    minimum_area: float | None
    minimum_volume: float | None
    minimum_voxels: int | None
    @staticmethod
    def area(square_angstrom: float) -> SurfaceComponentThreshold: ...
    @staticmethod
    def volume(cubic_angstrom: float) -> SurfaceComponentThreshold: ...
    @staticmethod
    def voxels(count: int) -> SurfaceComponentThreshold: ...
    def __repr__(self) -> str: ...

@final
class EntityProvenance:
    detail: ProvenanceDetail
    entity: EntityRef
    entry_id: str | None
    entry_title: str | None
    method: str | None
    ownership: MemoryOwnership
    resolution: float | None
    @staticmethod
    def copy_from_scene(scene: Scene, entity: EntityRef) -> EntityProvenance | None: ...

@final
class ProvenanceDetail:
    Annotation: ProvenanceDetail
    Atom: ProvenanceDetail
    Bond: ProvenanceDetail
    DynamicBond: ProvenanceDetail
    Guide: ProvenanceDetail
    Interaction: ProvenanceDetail
    Measurement: ProvenanceDetail
    Mesh: ProvenanceDetail
    Primitive: ProvenanceDetail
    Unknown: ProvenanceDetail
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class RepresentationConfig:
    def __new__(cls, kind: RepresentationKind) -> RepresentationConfig: ...
    kind: RepresentationKind
    def radius_scale(self, scale: float) -> RepresentationConfig: ...
    def bond_radius(self, radius: float) -> RepresentationConfig: ...
    def tube_radius(self, radius: float) -> RepresentationConfig: ...
    def putty_b_factor(self, domain: tuple[float, float], radii: tuple[float, float]) -> RepresentationConfig: ...
    def isolevel(self, level: float) -> RepresentationConfig: ...
    def surface(self, kind: SurfaceKind, style: SurfaceStyle) -> RepresentationConfig: ...

@final
class RepresentationInput:
    kind: str
    @staticmethod
    def stored(selection: SelectionHandle) -> RepresentationInput: ...
    @staticmethod
    def query(query: Select) -> RepresentationInput: ...
    @staticmethod
    def source(source: str) -> RepresentationInput: ...
    @staticmethod
    def volume(volume: VolumeHandle) -> RepresentationInput: ...
    @staticmethod
    def segmentation(segmentation: SegmentationHandle) -> RepresentationInput: ...

@final
class RepresentationParams:
    def __new__(cls, radius_scale: float = 1.0, bond_radius: float = 0.18, probe_radius: float = 1.4, gaussian_sigma: float = 1.0, isolevel: float = 1.0, surface_kind: SurfaceKind = ..., surface_style: SurfaceStyle = ..., surface_components: SurfaceComponentPolicy | None = None, surface_pattern_spacing: float = 1.5, surface_pattern_width_pixels: float = 1.25, ribbon_width: float = 1.2, tube_radius: float = 0.3, tube_radius_mapping: TubeRadiusMapping | None = None, point_size_pixels: float = 3.0, line_width_pixels: float = 1.5) -> RepresentationParams: ...
    bond_radius: float
    isolevel: float
    probe_radius: float
    radius_scale: float
    surface_components: SurfaceComponentPolicy
    tube_radius: float
    @staticmethod
    def default() -> RepresentationParams: ...

@final
class AtomSelection:
    @staticmethod
    def empty() -> AtomSelection: ...
    @staticmethod
    def all() -> AtomSelection: ...
    @staticmethod
    def range(start: int, end: int) -> AtomSelection: ...
    @staticmethod
    def copy_sparse(rows: Sequence[int]) -> AtomSelection: ...
    def count(self, table_len: int) -> int: ...
    def contains(self, row: int) -> bool: ...
    def union(self, other: AtomSelection, table_len: int) -> AtomSelection: ...
    def intersect(self, other: AtomSelection, table_len: int) -> AtomSelection: ...
    def difference(self, other: AtomSelection, table_len: int) -> AtomSelection: ...
    def copy_rows(self, table_len: int) -> NDArray[np.uint32]: ...

@final
class VisualInstructionGpu:
    def __new__(cls, control: tuple[int, int, int, int], data: tuple[float, float, float, float]) -> VisualInstructionGpu: ...
    control: list[int]
    data: list[float]
    @staticmethod
    def byte_size() -> int: ...

@final
class VolumeRendering:
    Direct: VolumeRendering
    Isosurface: VolumeRendering
    LiquidSurface: VolumeRendering
    Medium: VolumeRendering
    Slice: VolumeRendering
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class VolumeTransferFunction:
    def __new__(cls, points: Sequence[VolumeTransferPoint]) -> VolumeTransferFunction: ...
    @staticmethod
    def linear(range: tuple[float, float], low: Rgba8, high: Rgba8) -> VolumeTransferFunction: ...
    @staticmethod
    def default() -> VolumeTransferFunction: ...
    def copy_points(self) -> list[VolumeTransferPoint]: ...

@final
class VolumeTransferPoint:
    def __new__(cls, value: float, color: Rgba8, opacity: float) -> VolumeTransferPoint: ...
    color: Rgba8
    opacity: float
    value: float

@final
class VisualInputs:
    def __new__(cls,
        base_color: tuple[float, float, float, float] = ...,
        base_opacity: float = 1.0,
        time_seconds: float = 0.0,
        local_position: tuple[float, float, float] = ...,
        world_position: tuple[float, float, float] = ...,
        normal: tuple[float, float, float] = ...,
        view_direction: tuple[float, float, float] = ...,
        camera_distance: float = 0.0,
        entity_index: int = 0,
        roughness: float = 0.34,
        specular: float = 0.5,
        material_strength: float = 0.0,
        properties: tuple[float, float, float, float] | None = None,
    ) -> VisualInputs: ...

@final
class VisualEvaluation:
    base_color: tuple[float, float, float, float]
    opacity: float
    emission: tuple[float, float, float]
    roughness: float
    specular: float
    material_strength: float
    visible: bool
    silhouette_softness: float
    radius_scale: float
    width_scale: float
    position_offset: tuple[float, float, float]

@final
class VisualAttributeRef:
    @staticmethod
    def attribute(handle: AttributeHandle, kind: AttributeKind) -> VisualAttributeRef: ...
    @staticmethod
    def column(key: VisualColumnKey, kind: AttributeKind) -> VisualAttributeRef: ...
    @staticmethod
    def legacy_scalar(property: AtomPropertyHandle) -> VisualAttributeRef: ...
    handle: AttributeHandle | None
    key: VisualColumnKey | None
    kind: AttributeKind
    property: AtomPropertyHandle | None
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class VisualColumnKey:
    def __new__(cls, value: int) -> VisualColumnKey: ...
    value: int
    def __eq__(self, other: object, /) -> bool: ...
    def __int__(self) -> int: ...
    def __repr__(self) -> str: ...

@final
class VisualDescriptor:
    order: int
    style: VisualStyle
    def __repr__(self) -> str: ...

@final
class VisualProgram:
    fingerprint: int
    instruction_count: int
    uniform_instruction_count: int
    entity_instruction_count: int
    fragment_instruction_count: int
    maximum_displacement: float
    def attributes(self) -> list[VisualAttributeRef]: ...
    def scalar_parameter(self, index: int) -> ScalarParameter | None: ...
    def color_parameter(self, index: int) -> ColorParameter | None: ...
    def vector_parameter(self, index: int) -> VectorParameter | None: ...
    def evaluate(self, inputs: VisualInputs) -> VisualEvaluation: ...
    def __repr__(self) -> str: ...

@final
class VisualStyle:
    def __new__(cls, program: VisualProgram) -> VisualStyle: ...
    program: VisualProgram
    @staticmethod
    def pulse(color: Rgba8, cycles_per_second: float, minimum: float, maximum: float) -> VisualStyle: ...
    def set_scalar(self, parameter: ScalarParameter, value_: float) -> None: ...
    def set_color(self, parameter: ColorParameter, value_: tuple[float, float, float, float]) -> None: ...
    def set_vector(self, parameter: VectorParameter, value_: tuple[float, float, float]) -> None: ...
    def evaluate(self, inputs: VisualInputs) -> VisualEvaluation: ...

@final
class VisualProgramBuilder:
    def __new__(cls) -> VisualProgramBuilder: ...
    def scalar(self, value_: float) -> ScalarExpr: ...
    def color(self, value_: tuple[float, float, float, float]) -> ColorExpr: ...
    def vector(self, value_: tuple[float, float, float]) -> VectorExpr: ...
    def boolean(self, value_: bool) -> BoolExpr: ...
    def base_color(self) -> ColorExpr: ...
    def base_opacity(self) -> ScalarExpr: ...
    def base_roughness(self) -> ScalarExpr: ...
    def base_specular(self) -> ScalarExpr: ...
    def base_material_strength(self) -> ScalarExpr: ...
    def time(self) -> ScalarExpr: ...
    def local_position(self) -> VectorExpr: ...
    def world_position(self) -> VectorExpr: ...
    def normal(self) -> VectorExpr: ...
    def view_direction(self) -> VectorExpr: ...
    def camera_distance(self) -> ScalarExpr: ...
    def entity_index(self) -> ScalarExpr: ...
    def scalar_attribute(self, attribute: AttributeHandle) -> ScalarExpr: ...
    def category_attribute(self, attribute: AttributeHandle) -> ScalarExpr: ...
    def vector_attribute(self, attribute: AttributeHandle) -> VectorExpr: ...
    def color_attribute(self, attribute: AttributeHandle) -> ColorExpr: ...
    def scalar_column(self, key: VisualColumnKey) -> ScalarExpr: ...
    def category_column(self, key: VisualColumnKey) -> ScalarExpr: ...
    def vector_column(self, key: VisualColumnKey) -> VectorExpr: ...
    def color_column(self, key: VisualColumnKey) -> ColorExpr: ...
    def scalar_parameter(self, default: float) -> tuple[ScalarParameter, ScalarExpr]: ...
    def color_parameter(self, default: tuple[float, float, float, float]) -> tuple[ColorParameter, ColorExpr]: ...
    def vector_parameter(self, default: tuple[float, float, float]) -> tuple[VectorParameter, VectorExpr]: ...
    def add(self, a: ScalarExpr, b: ScalarExpr) -> ScalarExpr: ...
    def subtract(self, a: ScalarExpr, b: ScalarExpr) -> ScalarExpr: ...
    def multiply(self, a: ScalarExpr, b: ScalarExpr) -> ScalarExpr: ...
    def safe_divide(self, a: ScalarExpr, b: ScalarExpr) -> ScalarExpr: ...
    def abs(self, value_: ScalarExpr) -> ScalarExpr: ...
    def minimum(self, a: ScalarExpr, b: ScalarExpr) -> ScalarExpr: ...
    def maximum(self, a: ScalarExpr, b: ScalarExpr) -> ScalarExpr: ...
    def clamp(self, value_: ScalarExpr, low: ScalarExpr, high: ScalarExpr) -> ScalarExpr: ...
    def saturate(self, value_: ScalarExpr) -> ScalarExpr: ...
    def step(self, edge: ScalarExpr, value_: ScalarExpr) -> ScalarExpr: ...
    def smoothstep(self, low: ScalarExpr, high: ScalarExpr, value_: ScalarExpr) -> ScalarExpr: ...
    def sine(self, value_: ScalarExpr) -> ScalarExpr: ...
    def mix_scalar(self, a: ScalarExpr, b: ScalarExpr, weight: ScalarExpr) -> ScalarExpr: ...
    def mix_color(self, a: ColorExpr, b: ColorExpr, weight: ScalarExpr) -> ColorExpr: ...
    def ramp(self, value_: ScalarExpr, ramp: ScalarRamp) -> ColorExpr: ...
    def less(self, a: ScalarExpr, b: ScalarExpr) -> BoolExpr: ...
    def greater(self, a: ScalarExpr, b: ScalarExpr) -> BoolExpr: ...
    def and_(self, a: BoolExpr, b: BoolExpr) -> BoolExpr: ...
    def or_(self, a: BoolExpr, b: BoolExpr) -> BoolExpr: ...
    def not_(self, value_: BoolExpr) -> BoolExpr: ...
    def select_scalar(self, condition: BoolExpr, yes: ScalarExpr, no: ScalarExpr) -> ScalarExpr: ...
    def select_color(self, condition: BoolExpr, yes: ColorExpr, no: ColorExpr) -> ColorExpr: ...
    def add_vector(self, a: VectorExpr, b: VectorExpr) -> VectorExpr: ...
    def scale_vector(self, vector: VectorExpr, scale: ScalarExpr) -> VectorExpr: ...
    def dot(self, a: VectorExpr, b: VectorExpr) -> ScalarExpr: ...
    def normalize(self, value_: VectorExpr) -> VectorExpr: ...
    def set_base_color(self, value_: ColorExpr) -> None: ...
    def set_opacity(self, value_: ScalarExpr) -> None: ...
    def set_emission(self, value_: ColorExpr) -> None: ...
    def set_roughness(self, value_: ScalarExpr) -> None: ...
    def set_specular(self, value_: ScalarExpr) -> None: ...
    def set_material_strength(self, value_: ScalarExpr) -> None: ...
    def set_visibility(self, value_: BoolExpr) -> None: ...
    def set_silhouette_softness(self, value_: ScalarExpr) -> None: ...
    def set_radius_scale(self, value_: ScalarExpr) -> None: ...
    def set_width_scale(self, value_: ScalarExpr) -> None: ...
    def set_position_offset(self, value_: VectorExpr, maximum_displacement: float) -> None: ...
    def finish(self) -> VisualProgram: ...
    def finish_color(self, value_: ColorExpr) -> VisualProgram: ...

@final
class VisualCompatibility:
    @staticmethod
    def none() -> VisualCompatibility: ...
    @staticmethod
    def appearance() -> VisualCompatibility: ...
    @staticmethod
    def analytic_appearance() -> VisualCompatibility: ...
    @staticmethod
    def sized() -> VisualCompatibility: ...
    @staticmethod
    def deformable() -> VisualCompatibility: ...
    @staticmethod
    def ribbon() -> VisualCompatibility: ...
    @staticmethod
    def for_representation(kind: RepresentationKind) -> VisualCompatibility: ...
    def supports(self, output: VisualOutput) -> bool: ...

@final
class VisualOutput:
    BaseColor: VisualOutput
    Opacity: VisualOutput
    Emission: VisualOutput
    Roughness: VisualOutput
    Specular: VisualOutput
    MaterialStrength: VisualOutput
    Visibility: VisualOutput
    SilhouetteSoftness: VisualOutput
    RadiusScale: VisualOutput
    WidthScale: VisualOutput
    PositionOffset: VisualOutput
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class VisualStage:
    Uniform: VisualStage
    Entity: VisualStage
    Fragment: VisualStage
    def __eq__(self, other: object, /) -> bool: ...
    def __repr__(self) -> str: ...

@final
class ScalarExpr:
    ...

@final
class ColorExpr:
    ...

@final
class BoolExpr:
    ...

@final
class VectorExpr:
    ...

@final
class ScalarParameter:
    ...

@final
class ColorParameter:
    ...

@final
class VectorParameter:
    ...

def cpk_color(atomic_number: int) -> Rgba8:
    """CPK colour of one element, falling back to the unknown-element colour."""


def select(source: str) -> Select:
    """Parses a selection expression into a :class:`Select`."""


def vdw_radius(atomic_number: int) -> float:
    """Radius of one element in ångström, falling back to hydrogen's."""


MAX_CLIP_PLANES: int
MAX_MESH_VERTICES: int
MAX_VISUAL_INSTRUCTIONS: int
MAX_VISUAL_PARAMETERS: int
MAX_VISUAL_PROPERTIES: int
MAX_VOLUME_TRANSFER_POINTS: int
