"""Native declarative MolGFX bindings."""

from collections.abc import Callable, Sequence
from os import PathLike
from types import TracebackType
from typing import Literal, Protocol, Self, final, overload

class _QueryLike(Protocol):
    @property
    def source(self) -> str: ...

type _Target = str | _QueryLike
type _Rgb = tuple[int, int, int]
type _CartoonStyle = Literal["ribbon", "rocket", "nucleic_acid", "glycan"]
type _SurfaceKind = Literal["van_der_waals", "solvent_accessible", "solvent_excluded", "gaussian"]
type _SurfaceStyle = Literal["solid", "contour", "dots", "filled_contour", "mesh", "soft_union"]

@final
class RepresentationId:
    @property
    def value(self) -> int: ...
    def __int__(self) -> int: ...
    def __index__(self) -> int: ...

@final
class StructureId:
    @property
    def value(self) -> int: ...
    def __int__(self) -> int: ...
    def __index__(self) -> int: ...

@final
class VolumeId:
    @property
    def value(self) -> int: ...
    def __int__(self) -> int: ...
    def __index__(self) -> int: ...

@final
class SegmentationId:
    @property
    def index(self) -> int: ...
    @property
    def generation(self) -> int: ...

@final
class AnnotationId:
    @property
    def value(self) -> int: ...
    def __int__(self) -> int: ...
    def __index__(self) -> int: ...

@final
class MeasurementId:
    @property
    def value(self) -> int: ...
    def __int__(self) -> int: ...
    def __index__(self) -> int: ...

@final
class InteractionId:
    @property
    def value(self) -> int: ...
    def __int__(self) -> int: ...
    def __index__(self) -> int: ...

@final
class TrajectoryId:
    @property
    def value(self) -> int: ...
    def __int__(self) -> int: ...
    def __index__(self) -> int: ...

@final
class EllipsoidId:
    @property
    def value(self) -> int: ...
    def __int__(self) -> int: ...
    def __index__(self) -> int: ...

@final
class DataSource: ...

@final
class Anchor: ...

@final
class Volume:
    def isosurface(
        self,
        isovalue: float,
        *,
        color: _Rgb = ...,
        opacity: float = 1.0,
        style: Literal["solid", "mesh", "dots"] = "solid",
        width: float = 0.1,
    ) -> Self: ...
    def direct(
        self,
        transfer: Sequence[tuple[float, _Rgb, float]],
        *,
        opacity_scale: float = 2.0,
        step_scale: float = 0.65,
        medium: bool = False,
    ) -> Self: ...
    def slice(
        self,
        *,
        point: tuple[float, float, float],
        normal: tuple[float, float, float],
        ramp: str = "viridis",
        domain: tuple[float, float] = ...,
    ) -> Self: ...
    def liquid_surface(
        self, isovalue: float, *, color: _Rgb = ..., opacity: float = 1.0
    ) -> Self: ...
    def region(self, minimum: tuple[int, int, int], maximum: tuple[int, int, int]) -> Self: ...

@final
class Segmentation:
    @staticmethod
    def from_json(source: str) -> Segmentation: ...
    def to_json(self) -> str: ...

@final
class SegmentStyle: ...

@final
class Label: ...

@final
class Measurement: ...

@final
class Interaction: ...

@final
class Trajectory: ...

@final
class Ellipsoid: ...

@final
class ColorSpec:
    def legend_json(self) -> str | None: ...

type _ColorLike = _Rgb | ColorSpec
type _Category = Literal[
    "chain", "entity", "molecule_type", "residue_name", "residue", "secondary_structure"
]
type _Metric = Literal[
    "occupancy",
    "b_factor",
    "formal_charge",
    "hydrophobicity",
    "sequence_position",
    "sasa",
    "plddt",
    # `rainbow` is the sequence sweep and `confidence` the pLDDT bands; neither
    # is a second metric, so a column still binds under one name.
    "rainbow",
    "confidence",
]

@final
class ScalarProperty: ...

@final
class ScalarExpr:
    def __add__(self, right: _ScalarLike, /) -> ScalarExpr: ...
    def __radd__(self, left: _ScalarLike, /) -> ScalarExpr: ...
    def __mul__(self, right: _ScalarLike, /) -> ScalarExpr: ...
    def __rmul__(self, left: _ScalarLike, /) -> ScalarExpr: ...
    def clamp(self, minimum: float, maximum: float) -> ScalarExpr: ...
    def less(self, right: _ScalarLike) -> BoolExpr: ...

@final
class ScalarParameter:
    @property
    def name(self) -> str: ...
    @property
    def default(self) -> float: ...
    def expression(self) -> ScalarExpr: ...

type _ScalarLike = float | ScalarExpr | ScalarParameter

@final
class VectorExpr:
    def __add__(self, right: _VectorLike, /) -> VectorExpr: ...
    def __radd__(self, left: _VectorLike, /) -> VectorExpr: ...
    def __mul__(self, right: _ScalarLike, /) -> VectorExpr: ...
    def __rmul__(self, left: _ScalarLike, /) -> VectorExpr: ...
    def normalized(self) -> VectorExpr: ...
    def dot(self, right: _VectorLike) -> ScalarExpr: ...

@final
class VectorParameter:
    @property
    def name(self) -> str: ...
    @property
    def default(self) -> tuple[float, float, float]: ...
    def expression(self) -> VectorExpr: ...

type _VectorLike = tuple[float, float, float] | VectorExpr | VectorParameter

@final
class BoolExpr:
    def __and__(self, right: BoolExpr, /) -> BoolExpr: ...
    def __rand__(self, left: BoolExpr, /) -> BoolExpr: ...
    def __or__(self, right: BoolExpr, /) -> BoolExpr: ...
    def __ror__(self, left: BoolExpr, /) -> BoolExpr: ...
    def __invert__(self) -> BoolExpr: ...

type _BoolLike = bool | BoolExpr

@final
class ColorExpr: ...

@final
class ColorParameter:
    @property
    def name(self) -> str: ...
    @property
    def default(self) -> _Rgb: ...
    def expression(self) -> ColorExpr: ...

type _ColorExprLike = _Rgb | ColorExpr | ColorParameter

@final
class VisualStyle:
    def stable_hash(self) -> str: ...
    def explain(self) -> str: ...
    def wgsl(self) -> str: ...

class MolgfxError(Exception): ...
class SpecError(MolgfxError): ...
class RevisionConflict(MolgfxError): ...

@final
class Camera:
    def __new__(
        cls,
        *,
        position: tuple[float, float, float],
        target: tuple[float, float, float],
        up: tuple[float, float, float] = ...,
        fov_y: float | None = None,
        aspect: float = 1.0,
        near: float = 0.1,
        far: float = 10_000.0,
    ) -> Self: ...
    @property
    def position(self) -> tuple[float, float, float]: ...
    @property
    def target(self) -> tuple[float, float, float]: ...
    @property
    def up(self) -> tuple[float, float, float]: ...
    def project(
        self, point: tuple[float, float, float], size: tuple[int, int]
    ) -> tuple[float, float, float] | None:
        """Return pixels from the top-left and distance from the eye, or ``None`` behind it."""
    def ray(
        self, x: float, y: float, size: tuple[int, int]
    ) -> tuple[tuple[float, float, float], tuple[float, float, float]] | None:
        """Return the origin and unit direction of the ray through a pixel."""

@final
class Representation:
    def explain(self) -> str: ...
    def visual(self, style: VisualStyle) -> Representation: ...
    def on(self, structure: StructureId) -> Representation: ...

@final
class SceneSpec:
    """Canonical JSON scene state, including planes and assembly unit-cell guides."""

    def __new__(cls, source: str) -> Self: ...
    @property
    def revision(self) -> int: ...
    def to_json(self) -> str: ...
    def stable_hash(self) -> str: ...
    def patched(self, patch: ScenePatch) -> SceneSpec: ...

@final
class ScenePatch:
    """Revision-checked JSON edits, including add_plane/remove_plane/set_assembly."""

    def __new__(cls, source: str) -> Self: ...
    @property
    def base_revision(self) -> int: ...
    def to_json(self) -> str: ...
    def inverse(self, base: SceneSpec) -> ScenePatch: ...

@final
class SceneTransaction:
    def __enter__(self) -> Scene: ...
    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> Literal[False]: ...

@final
class Scene:
    def __new__(cls, structure: object | None = None) -> Self: ...
    @overload
    def add(self, item: Representation) -> RepresentationId: ...
    @overload
    def add(self, item: Volume) -> VolumeId: ...
    @overload
    def add(self, item: Segmentation) -> SegmentationId: ...
    @overload
    def add(self, item: Label) -> AnnotationId: ...
    @overload
    def add(self, item: Measurement) -> MeasurementId: ...
    @overload
    def add(self, item: Interaction) -> InteractionId: ...
    @overload
    def add(self, item: Trajectory) -> TrajectoryId: ...
    @overload
    def add(self, item: Ellipsoid) -> EllipsoidId: ...
    def add_structure(self, structure: object) -> StructureId: ...
    def auto(self, *, structure: StructureId | None = None) -> Sequence[RepresentationId]: ...
    def ensemble(
        self,
        members: Sequence[tuple[StructureId, float, tuple[int, int, int]]],
        *,
        dominant_opacity: float | None = None,
        alternate_opacity: float | None = None,
        minimum_opacity: float | None = None,
    ) -> Sequence[RepresentationId]: ...
    def difference(
        self,
        property: ScalarProperty,
        target: _Target,
        *,
        structure: StructureId | None = None,
        style: DifferenceStyle | None = None,
    ) -> RepresentationId: ...
    def place(
        self, matrix: Sequence[float], *, structure: StructureId | None = None
    ) -> StructureId: ...
    def assembly(
        self, instances: Sequence[object], *, of: StructureId | None = None
    ) -> list[AssemblyCopy]: ...
    def pocket(
        self,
        focus: _Target,
        *,
        structure: StructureId | None = None,
        style: PocketStyle | None = None,
    ) -> Sequence[RepresentationId]: ...
    def bind_property(
        self,
        *,
        structure: StructureId,
        name: str,
        source_hash: str,
        values: Sequence[float],
        units: str | None = None,
        domain: tuple[float, float] | None = None,
    ) -> ScalarProperty: ...
    def bind_trajectory(
        self,
        *,
        source_hash: str,
        start: TrajectoryFrame,
        end: TrajectoryFrame,
        sample_time: float | None = None,
    ) -> None: ...
    def set_trajectory_time(self, *, structure: StructureId, seconds: float) -> None: ...
    def bind_volume(self, identity: VolumeId, values: list[float]) -> None: ...
    def bind_segmentation(self, identity: SegmentationId, labels: Sequence[int]) -> None: ...
    def set_segment_styles(
        self, identity: SegmentationId, styles: Sequence[SegmentStyle]
    ) -> None: ...
    def remove_segmentation(self, identity: SegmentationId) -> None: ...
    def resolve_pick(self, pick: PickResult) -> str: ...
    def unresolved_overlays(self) -> str: ...
    def set_visible(self, representation: RepresentationId, visible: bool) -> None: ...
    def set_opacity(self, representation: RepresentationId, opacity: float) -> None: ...
    def set_visual(self, representation: RepresentationId, visual: VisualStyle | None) -> None: ...
    @overload
    def set_parameter(
        self, representation: RepresentationId, parameter: ScalarParameter, value: float
    ) -> None: ...
    @overload
    def set_parameter(
        self,
        representation: RepresentationId,
        parameter: VectorParameter,
        value: tuple[float, float, float],
    ) -> None: ...
    @overload
    def set_parameter(
        self, representation: RepresentationId, parameter: ColorParameter, value: _Rgb
    ) -> None: ...
    def focus(self, target: _Target) -> None: ...
    def frame(self, target: _Target, *, aspect: float = 1.0) -> Camera:
        """Return a camera framing a selection as atom spheres, leaving the scene as it is."""
    def selection_bounds(
        self, target: _Target
    ) -> tuple[tuple[float, float, float], tuple[float, float, float]] | None:
        """Return the minimum and maximum corners of a selection in world space."""
    def set_camera(self, camera: Camera | None) -> None: ...
    def transaction(self) -> SceneTransaction: ...
    @overload
    def set_interaction(
        self,
        *,
        channel: Literal["selected", "hovered", "focused", "muted", "hidden"],
        target: _Target | None = None,
        name: None = None,
    ) -> None: ...
    @overload
    def set_interaction(
        self, *, channel: Literal["custom"], target: _Target | None = None, name: str
    ) -> None: ...
    def apply(self, patch: ScenePatch) -> None: ...
    @property
    def revision(self) -> int: ...
    @property
    def structure_id(self) -> StructureId: ...
    def to_json(self) -> str: ...
    @property
    def spec(self) -> SceneSpec: ...
    def explain(self) -> str: ...
    def browser_sources(self) -> list[tuple[int, str, bytes]]:
        """Return compact structure payloads; encoding is cached by the scene."""
    def subscribe(self, subscriber: Callable[[], Callable[[str], object] | None]) -> None:
        """Register a weak callable receiving each committed patch as JSON."""
    def unsubscribe(self, subscriber: Callable[[], Callable[[str], object] | None]) -> None:
        """Remove the exact weak subscription previously registered."""

class CommandError(MolgfxError):
    errors: list[dict[str, object]]

type _Form = Literal[
    "cartoon",
    "ball_and_stick",
    "spacefill",
    "licorice",
    "lines",
    "points",
    "surface",
    "nucleic_acid",
    "bases",
    "base_pairs",
    "glycan",
    "beads",
    "dots",
    "backbone",
    "trace",
    "tube",
    "putty",
]

@final
class Command:
    @staticmethod
    def parse(text: str) -> Command: ...
    @staticmethod
    def from_json(source: str) -> Command: ...
    @staticmethod
    def select(selection: str, target: _Target) -> Command: ...
    @staticmethod
    def unselect(selection: str) -> Command: ...
    @staticmethod
    def set_selection(target: _Target | None = None) -> Command:
        """Atomically replace or clear current selection through Session history."""
    @staticmethod
    def show(form: _Form | str, target: _Target, **options: object) -> Command: ...
    @staticmethod
    def reveal(layer: str) -> Command: ...
    @staticmethod
    def hide(layer: str) -> Command: ...
    @staticmethod
    def remove(layer: str) -> Command: ...
    @staticmethod
    def color(color: str, target: _Target, *, structure: str | None = None) -> Command: ...
    @staticmethod
    def uncolor(target: _Target | None = None, *, structure: str | None = None) -> Command: ...
    @staticmethod
    def opacity(value: float, layer: str) -> Command: ...
    @staticmethod
    def focus(target: _Target) -> Command: ...
    @staticmethod
    def auto(*, structure: str | None = None) -> Command: ...
    @staticmethod
    def pocket(
        target: _Target,
        *,
        near: float | None = None,
        mid: float | None = None,
        structure: str | None = None,
    ) -> Command: ...
    @staticmethod
    def unfocus() -> Command: ...
    @staticmethod
    def undo() -> Command: ...
    @staticmethod
    def redo() -> Command: ...
    @property
    def verb(self) -> str: ...
    def to_json(self) -> str: ...
    def __eq__(self, value: object, /) -> bool: ...

@final
class CommandResult:
    @property
    def revision(self) -> int: ...
    @property
    def messages(self) -> list[str]: ...
    @property
    def patch(self) -> ScenePatch | None: ...

@final
class Session:
    def __new__(cls, source: Scene | object) -> Self: ...
    @property
    def scene(self) -> Scene: ...
    def execute(self, program: str | Command | Sequence[Command]) -> CommandResult: ...
    @staticmethod
    def explain(text: str) -> str: ...
    def undo(self) -> CommandResult: ...
    def redo(self) -> CommandResult: ...
    def snapshot_save(self, name: str) -> CommandResult: ...
    def snapshot_restore(self, name: str) -> CommandResult: ...
    def snapshot_remove(self, name: str) -> CommandResult: ...
    @property
    def can_undo(self) -> bool: ...
    @property
    def can_redo(self) -> bool: ...
    @property
    def history(self) -> list[str]: ...
    @property
    def selections(self) -> dict[str, str]: ...
    @property
    def layers(self) -> dict[str, tuple[str, str, int]]: ...
    @property
    def structures(self) -> dict[str, int]: ...
    def completions(self, text: str, cursor: int | None = None) -> list[tuple[str, str, str]]: ...
    def explain_selection(self, name: str) -> str: ...
    def explain_layer(self, name: str) -> str: ...
    def summary(self) -> str: ...
    def to_json(self) -> str: ...
    @staticmethod
    def from_json(scene: Scene, source: str) -> Session: ...

def vocabulary() -> dict[str, object]: ...

@final
class Image:
    @property
    def width(self) -> int: ...
    @property
    def height(self) -> int: ...
    def quality_json(self) -> str: ...
    def pixels(self) -> bytes: ...
    def png_bytes(self) -> bytes: ...
    def save(self, path: str | PathLike[str]) -> None: ...

@final
class HdrImage:
    """Converged scene-linear RGBA16F, without bloom or display processing."""

    @property
    def width(self) -> int: ...
    @property
    def height(self) -> int: ...
    def quality_json(self) -> str: ...
    def rgba16f(self) -> bytes:
        """Return row-major RGBA as little-endian IEEE binary16 words."""
    def exr_bytes(self) -> bytes:
        """Encode uncompressed half-float RGBA OpenEXR."""
    def save(self, path: str | PathLike[str]) -> None:
        """Write OpenEXR; reject other filename extensions with ValueError."""

@final
class PickResult:
    @property
    def kind(
        self,
    ) -> Literal[
        "atom",
        "bond",
        "interaction",
        "label",
        "measurement",
        "primitive",
        "mesh",
        "ligand_pose_batch",
        "guide",
        "dynamic_bond",
        "point",
        "instance",
        "template_part",
        "relation",
        "volume_segment",
    ]: ...
    @property
    def dataset(self) -> int | None: ...
    @property
    def chunk(self) -> int | None: ...
    @property
    def row(self) -> int | None: ...
    @property
    def volume_label(self) -> int | None: ...
    @property
    def source_id(self) -> int | None: ...
    @property
    def segmentation(self) -> str | None: ...

@final
class Effect:
    @property
    def kind(
        self,
    ) -> Literal[
        "depth_cue",
        "anti_aliasing",
        "bloom",
        "depth_of_field",
        "motion_blur",
        "backdrop",
        "lighting",
        "shape_cues",
        "display",
    ]: ...

@final
class RenderProfile:
    @property
    def target_fps(self) -> int: ...
    @property
    def quality(self) -> Literal["auto", "interactive", "highest_fixed", "converged"]: ...
    def with_effect(self, effect: Effect) -> Self: ...
    def without_effect(self, effect: Effect) -> Self: ...
    def effect(self, effect: Effect) -> Effect | None: ...

class _Effect:
    @staticmethod
    def depth_cue(*, near_distance: float, far_distance: float, strength: float) -> Effect: ...
    @staticmethod
    def fxaa() -> Effect: ...
    @staticmethod
    def no_anti_aliasing() -> Effect: ...
    @staticmethod
    def bloom(
        *, threshold: float = 1.7, intensity: float = 0.26, radius: float = 2.0
    ) -> Effect: ...
    @staticmethod
    def depth_of_field(
        *,
        focal_length_mm: float = 50.0,
        f_number: float = 8.0,
        sensor_width_mm: float = 36.0,
        max_blur_pixels: float = 14.0,
        blade_count: int = 7,
        focus_distance: float | None = None,
    ) -> Effect: ...
    @staticmethod
    def motion_blur(*, shutter: float = 0.55, max_blur_pixels: float = 18.0) -> Effect: ...
    @staticmethod
    def backdrop_gradient(
        *,
        top: tuple[int, int, int] = (238, 241, 245),
        bottom: tuple[int, int, int] = (206, 214, 223),
        glow_color: tuple[int, int, int] = (255, 255, 255),
        glow_strength: float = 0.3,
    ) -> Effect: ...
    @staticmethod
    def lighting(
        *, key_strength: float = 1.35, fill_strength: float = 0.30, shadow_strength: float = 0.66
    ) -> Effect: ...
    @staticmethod
    def shape_cues(
        *,
        silhouette_strength: float = 0.65,
        cavity_strength: float = 0.35,
        depth_cue_strength: float = 0.15,
        posterize_levels: float = 0.0,
        motion_persistence: float = 0.0,
        outline_width: float = 0.0,
    ) -> Effect: ...
    @staticmethod
    def display(
        *,
        exposure_ev: float = 0.22,
        contrast: float = 1.18,
        saturation: float = 1.3,
        vignette_strength: float = 0.16,
        peak_luminance_nits: float = 100.0,
    ) -> Effect: ...

def system_info() -> dict[str, object]: ...

@final
class AssemblyCopy:
    @property
    def structure(self) -> StructureId: ...
    @property
    def chains(self) -> list[str]: ...
    @property
    def selection(self) -> str: ...

type _Button = Literal["left", "right", "middle"]

class _CameraController:
    def pointer_move(self, x: float, y: float, camera: Camera) -> Camera: ...
    def pointer_button(
        self, button_name: _Button, pressed: bool, x: float, y: float, camera: Camera
    ) -> Camera: ...
    def scroll(self, delta: float, camera: Camera) -> Camera: ...
    def pinch(self, scale: float, camera: Camera) -> Camera: ...

@final
class ArcballController(_CameraController):
    def __new__(cls) -> Self: ...

@final
class OrbitController(_CameraController):
    def __new__(cls) -> Self: ...

@final
class FlyController(_CameraController):
    def __new__(cls) -> Self: ...
    def key(
        self,
        name: Literal["forward", "backward", "left", "right", "up", "down"],
        pressed: bool,
        camera: Camera,
    ) -> Camera: ...
    def advance(self, camera: Camera, seconds: float, speed: float) -> Camera: ...

@final
class CameraPath:
    def __new__(
        cls,
        keyframes: Sequence[tuple[float, Camera]],
        *,
        easing: Literal["linear", "smooth_step"] = "smooth_step",
    ) -> Self: ...
    def sample(self, seconds: float) -> Camera | None: ...
    @property
    def range(self) -> tuple[float, float]: ...
    def __len__(self) -> int: ...

@final
class PocketStyle:
    def __new__(cls, *, near: float | None = None, mid: float | None = None) -> Self: ...
    @property
    def near(self) -> float: ...
    @property
    def mid(self) -> float: ...
    @property
    def pocket_opacity(self) -> float: ...
    @property
    def context_opacity(self) -> float: ...
    @property
    def solvent_opacity(self) -> float: ...
    def with_opacity(
        self,
        *,
        pocket: float | None = None,
        context: float | None = None,
        solvent: float | None = None,
    ) -> PocketStyle: ...
    def with_colors(
        self,
        *,
        pocket: tuple[int, int, int] | None = None,
        context: tuple[int, int, int] | None = None,
        solvent: tuple[int, int, int] | None = None,
    ) -> PocketStyle: ...

@final
class DifferenceStyle:
    def __new__(
        cls,
        *,
        thresholds: tuple[float, float] | None = None,
        context_opacity: float | None = None,
        palette: str | None = None,
        domain: tuple[float, float] | None = None,
        missing: tuple[int, int, int] | None = None,
    ) -> Self: ...
    @property
    def thresholds(self) -> tuple[float, float]: ...
    @property
    def context_opacity(self) -> float: ...
    @property
    def palette(self) -> str: ...
    @property
    def domain(self) -> tuple[float, float]: ...

@final
class Renderer:
    def __new__(
        cls,
        *,
        profile: RenderProfile | None = None,
        surface_memory_mib: int | None = None,
    ) -> Self: ...
    def render_image(self, scene: Scene, *, size: tuple[int, int]) -> Image: ...
    def render_hdr_image(self, scene: Scene, *, size: tuple[int, int]) -> HdrImage:
        """Render a complete HDR exposure using the scene framing camera."""
    def render_sequence(
        self, scene: Scene, *, size: tuple[int, int], fps: int, frames: int
    ) -> list[Image]: ...
    def render_camera_path(
        self,
        scene: Scene,
        path: CameraPath,
        *,
        size: tuple[int, int],
        fps: int,
    ) -> list[Image]: ...
    def pick(self, x: int, y: int) -> PickResult | None: ...
    def explain(self, scene: Scene) -> str: ...

class _Rep:
    def cartoon(
        self,
        *,
        target: _Target,
        width: float | None = None,
        style: _CartoonStyle | None = None,
        aspect_ratio: float | None = None,
        arrow_factor: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def ball_and_stick(
        self,
        *,
        target: _Target,
        radius: float | None = None,
        bond_radius: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def spacefill(
        self,
        *,
        target: _Target,
        radius: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def licorice(
        self,
        *,
        target: _Target,
        radius: float | None = None,
        bond_radius: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def lines(
        self,
        *,
        target: _Target,
        width: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def points(
        self,
        *,
        target: _Target,
        size: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def surface(
        self,
        *,
        target: _Target,
        kind: _SurfaceKind | None = None,
        style: _SurfaceStyle | None = None,
        probe_radius: float | None = None,
        isolevel: float | None = None,
        blob_spread: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def nucleic_acid(
        self,
        *,
        target: _Target,
        width: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def bases(
        self,
        *,
        target: _Target,
        radius: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def base_pairs(
        self,
        *,
        target: _Target,
        radius: float | None = None,
        bond_radius: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def glycan(
        self,
        *,
        target: _Target,
        width: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def beads(
        self,
        *,
        target: _Target,
        radius: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def dots(
        self,
        *,
        target: _Target,
        size: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def backbone(
        self,
        *,
        target: _Target,
        width: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def trace(
        self,
        *,
        target: _Target,
        radius: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def tube(
        self,
        *,
        target: _Target,
        radius: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...
    def putty(
        self,
        *,
        target: _Target,
        domain_min: float | None = None,
        domain_max: float | None = None,
        radius_min: float | None = None,
        radius_max: float | None = None,
        opacity: float = 1.0,
        color: _ColorLike | None = None,
    ) -> Representation: ...

class _Color:
    def element(self) -> ColorSpec: ...
    def chain(self) -> ColorSpec: ...
    def residue(self) -> ColorSpec: ...
    def secondary_structure(self) -> ColorSpec: ...
    def entity(self) -> ColorSpec: ...
    def molecule_type(self) -> ColorSpec: ...
    def residue_name(self) -> ColorSpec: ...
    def carbon_by_chain(self) -> ColorSpec: ...
    def category(
        self, by: _Category, *, palette: str | None = None, carbon_only: bool = False
    ) -> ColorSpec: ...
    def metric(
        self,
        name: _Metric,
        *,
        ramp: str | None = None,
        domain: tuple[float, float] | None = None,
    ) -> ColorSpec: ...
    def ramp_names(self) -> list[str]: ...
    def palette_names(self) -> list[str]: ...
    def uniform(self, rgb: _Rgb) -> ColorSpec: ...
    def property(
        self,
        *,
        property: ScalarProperty,
        ramp: str,
        domain: tuple[float, float],
        units: str | None = None,
        missing: _Rgb = (128, 128, 128),
    ) -> ColorSpec: ...

class _Profile:
    def interactive(self) -> RenderProfile: ...
    def converged(self) -> RenderProfile: ...
    def adaptive(self, target_fps: int) -> RenderProfile: ...
    def highest_fixed(self, target_fps: int) -> RenderProfile: ...

class _Visual:
    def scalar(self, value: float) -> ScalarExpr: ...
    def property(self, property: ScalarProperty) -> ScalarExpr: ...
    def parameter(self, default: float, *, name: str) -> ScalarParameter: ...
    def vector(self, value: tuple[float, float, float]) -> VectorExpr: ...
    def vector_property(self, name: str) -> VectorExpr: ...
    def vector_parameter(
        self, default: tuple[float, float, float], *, name: str
    ) -> VectorParameter: ...
    def color_parameter(self, default: _Rgb, *, name: str) -> ColorParameter: ...
    def state(self, name: str) -> BoolExpr: ...
    def color(self, rgb: _Rgb) -> ColorExpr: ...
    def ramp(
        self,
        value: _ScalarLike,
        *,
        palette: str,
        domain: tuple[float, float],
        missing: _Rgb = (128, 128, 128),
    ) -> ColorExpr: ...
    def where(self, condition: BoolExpr, yes: _ColorExprLike, no: _ColorExprLike) -> ColorExpr: ...
    def style(
        self,
        *,
        color: _ColorExprLike,
        opacity: _ScalarLike = 1.0,
        visible: _BoolLike = True,
    ) -> VisualStyle: ...

class _Data:
    def source(
        self, content_hash: str, *, uri: str | None = None, format: str | None = None
    ) -> DataSource: ...

class _Annotation:
    def world(self, position: tuple[float, float, float]) -> Anchor: ...
    def selection(self, *, structure: StructureId, target: _Target) -> Anchor: ...
    def label(self, *, anchor: Anchor, text: str, color: _Rgb = (255, 255, 255)) -> Label: ...

class _Segmentation:
    def style(
        self,
        label: int,
        *,
        color: _Rgb = (49, 104, 142),
        opacity: float = 1.0,
        visible: bool = True,
    ) -> SegmentStyle: ...
    def volume(
        self,
        *,
        source: DataSource,
        dimensions: tuple[int, int, int],
        styles: Sequence[SegmentStyle],
        voxel_to_world: Sequence[float] | None = None,
        presentation: Literal["surface", "direct"] = "surface",
    ) -> Segmentation: ...

class _Density:
    def volume(
        self,
        *,
        source: DataSource,
        dimensions: tuple[int, int, int],
        voxel_to_world: tuple[
            float,
            float,
            float,
            float,
            float,
            float,
            float,
            float,
            float,
            float,
            float,
            float,
            float,
            float,
            float,
            float,
        ]
        | None = None,
    ) -> Volume: ...

class _Measurement:
    def distance(self, first: Anchor, second: Anchor) -> Measurement: ...
    def angle(self, first: Anchor, middle: Anchor, last: Anchor) -> Measurement: ...
    def dihedral(
        self, first: Anchor, second: Anchor, third: Anchor, fourth: Anchor
    ) -> Measurement: ...

type _InteractionKind = Literal[
    "hydrogen_bond",
    "salt_bridge",
    "pi_stacking",
    "cation_pi",
    "hydrophobic",
    "metal_coordination",
    "contact",
]

class _Interaction:
    def explicit(self, *, kind: _InteractionKind, first: Anchor, second: Anchor) -> Interaction: ...

@final
class TrajectoryFrame:
    def __new__(cls, index: int, time: float, positions: Sequence[Sequence[float]]) -> Self: ...
    @property
    def index(self) -> int: ...
    @property
    def time(self) -> float: ...
    @property
    def positions(self) -> list[tuple[float, float, float]]: ...

class _Trajectory:
    def bind(
        self,
        *,
        structure: StructureId,
        source: DataSource,
        frame_count: int,
        time_step: float | None = None,
        time_unit: str | None = None,
    ) -> Trajectory: ...
    def frame(
        self, index: int, time: float, positions: Sequence[Sequence[float]]
    ) -> TrajectoryFrame: ...

class _Ellipsoid:
    def adp(
        self,
        *,
        structure: StructureId,
        target: _Target,
        scale: float | None = None,
        color: _ColorLike | None = None,
        opacity: float | None = None,
    ) -> Ellipsoid: ...

rep: _Rep
color: _Color
profile: _Profile
effect: _Effect
visual: _Visual
data: _Data
annotation: _Annotation
density: _Density
segmentation: _Segmentation
measurement: _Measurement
interaction: _Interaction
trajectory: _Trajectory
ellipsoid: _Ellipsoid
__version__: str
__all__ = [
    "Anchor",
    "AnnotationId",
    "ArcballController",
    "AssemblyCopy",
    "BoolExpr",
    "Camera",
    "CameraPath",
    "ColorExpr",
    "ColorParameter",
    "ColorSpec",
    "Command",
    "CommandError",
    "CommandResult",
    "DataSource",
    "DifferenceStyle",
    "Effect",
    "Ellipsoid",
    "EllipsoidId",
    "FlyController",
    "HdrImage",
    "Image",
    "Interaction",
    "InteractionId",
    "Label",
    "Measurement",
    "MeasurementId",
    "MolgfxError",
    "OrbitController",
    "PickResult",
    "PocketStyle",
    "RenderProfile",
    "Renderer",
    "Representation",
    "RepresentationId",
    "RevisionConflict",
    "ScalarExpr",
    "ScalarParameter",
    "ScalarProperty",
    "Scene",
    "ScenePatch",
    "SceneSpec",
    "SceneTransaction",
    "SegmentStyle",
    "Segmentation",
    "SegmentationId",
    "Session",
    "SpecError",
    "StructureId",
    "Trajectory",
    "TrajectoryFrame",
    "TrajectoryId",
    "VectorExpr",
    "VectorParameter",
    "VisualStyle",
    "Volume",
    "VolumeId",
    "__version__",
    "annotation",
    "color",
    "data",
    "density",
    "effect",
    "ellipsoid",
    "interaction",
    "measurement",
    "profile",
    "rep",
    "segmentation",
    "system_info",
    "trajectory",
    "visual",
    "vocabulary",
]
