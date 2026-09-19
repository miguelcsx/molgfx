"""The native module of :mod:`molgfx`.

The typed surface lives in four namespaces, each mirroring the Rust facade of
the same name: :mod:`molgfx._engine.core` is what ``molgfx::core`` is,
and so are ``math``, ``render`` and ``semantic``. The exceptions describe the
package as a whole, so they sit here.
"""

from . import core as core
from . import math as math
from . import render as render
from . import semantic as semantic

__version__: str

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
]


class MolgfxError(Exception): ...


class ChunkPlacementError(MolgfxError): ...


class ChunkResidencyError(MolgfxError): ...


class CoreError(MolgfxError): ...


class GpuError(MolgfxError): ...


class SemanticError(MolgfxError): ...


class RenderError(MolgfxError): ...


class VisualError(MolgfxError): ...


class ManifestError(CoreError): ...


class MappingError(SemanticError): ...


class AttributeRemovedError(AttributeError): ...
