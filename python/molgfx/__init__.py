"""A semantic, GPU-native molecular renderer.

The engine is written in Rust and this package is its typed surface; the
Python side holds no logic of its own. The root carries the curated names an
ordinary program wants, so a program reads ``molgfx.Scene``, not
``molgfx.core.Scene``:

```python
import molframe, molgfx as mg

scene = mg.Scene.from_structure_shared(molframe.read("1ubq.cif"))
scene.represent("polymer", mg.Representation.cartoon())
image = mg.Engine(mg.EngineConfig(1920, 1080)).render_image(scene, camera, 1920, 1080)
```

Subsystems stay namespaced the way they are in Rust, so a name lives in
exactly one place:

``molgfx.core``
    The scene graph — structures, selections, representations, values and
    authoring.
``molgfx.math``
    Vectors, quaternions, matrices, bounds and the camera.
``molgfx.render``
    The engine and the chunk and brick residency that feeds it.
``molgfx.semantic``
    Focus and context, level of detail, streaming policy and property mapping.

The exceptions sit here because they describe the package as a whole.
"""

from ._engine import (
    AttributeRemovedError,
    ChunkPlacementError,
    ChunkResidencyError,
    CoreError,
    GpuError,
    ManifestError,
    MappingError,
    MolgfxError,
    RenderError,
    SemanticError,
    VisualError,
    __version__,
)

from . import core, math, render, semantic
from .core import (
    AtomSelection,
    AttributeHandle,
    CameraBookmark,
    ColorScheme,
    EntityProvenance,
    Ensemble,
    EnsembleHandle,
    InstanceBatchHandle,
    Material,
    MeasurementHandle,
    MeshHandle,
    OverlayHandle,
    PointBatchHandle,
    PrimitiveHandle,
    RelationBatchHandle,
    Representation,
    RepresentationConfig,
    RepresentationHandle,
    RepresentationKind,
    RepresentationPreset,
    Scene,
    Select,
    SelectionHandle,
    StructureHandle,
    Timeline,
    TimelineTrackHandle,
    VolumeHandle,
)
from .math import (
    Aabb,
    Camera,
    Mat3,
    Mat4,
    Quat,
    Rgba8,
    Vec2,
    Vec3,
    Vec4,
)
from .render import (
    Engine,
    EngineConfig,
    FrameReport,
    Image,
    ImageConfig,
    Pick,
    RenderMode,
    RenderProfile,
)
from .semantic import (
    FocusScene,
    FocusStyle,
    FocusView,
    GenericCompositionScene,
    SurfaceZone,
    SurfaceZoneScene,
    SurfaceZoneStyle,
)

__all__ = [
    "AttributeRemovedError",
    "ChunkPlacementError",
    "ChunkResidencyError",
    "CoreError",
    "GpuError",
    "ManifestError",
    "MappingError",
    "MolgfxError",
    "RenderError",
    "SemanticError",
    "VisualError",
    "__version__",
    "core",
    "math",
    "render",
    "semantic",
    # Core.
    "AtomSelection",
    "AttributeHandle",
    "CameraBookmark",
    "ColorScheme",
    "EntityProvenance",
    "Ensemble",
    "EnsembleHandle",
    "InstanceBatchHandle",
    "Material",
    "MeasurementHandle",
    "MeshHandle",
    "OverlayHandle",
    "PointBatchHandle",
    "PrimitiveHandle",
    "RelationBatchHandle",
    "Representation",
    "RepresentationConfig",
    "RepresentationHandle",
    "RepresentationKind",
    "RepresentationPreset",
    "Scene",
    "Select",
    "SelectionHandle",
    "StructureHandle",
    "Timeline",
    "TimelineTrackHandle",
    "VolumeHandle",
    # Math.
    "Aabb",
    "Camera",
    "Mat3",
    "Mat4",
    "Quat",
    "Rgba8",
    "Vec2",
    "Vec3",
    "Vec4",
    # Render.
    "Engine",
    "EngineConfig",
    "FrameReport",
    "Image",
    "ImageConfig",
    "Pick",
    "RenderMode",
    "RenderProfile",
    # Semantic.
    "FocusScene",
    "FocusStyle",
    "FocusView",
    "GenericCompositionScene",
    "SurfaceZone",
    "SurfaceZoneScene",
    "SurfaceZoneStyle",
]


def __getattr__(name: str) -> None:
    raise AttributeRemovedError(
        f"{name!r} is not part of molgfx; the root carries the curated surface "
        "and every name lives in one namespace: molgfx.core, molgfx.math, "
        "molgfx.render or molgfx.semantic"
    )