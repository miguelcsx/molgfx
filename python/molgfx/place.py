"""Rigid-body placement helpers.

`Scene.place` takes a *column-major* 4x4 transform as 16 floats, while NumPy and
molframe use row-major matrices. These helpers perform that conversion once so
callers never transpose by hand.
"""

from collections.abc import Iterable
from typing import Protocol

from ._engine import Scene, StructureId

__all__ = ["apply", "from_rotation_translation", "from_superposition"]

_DIM = 3


class _Superposition(Protocol):
    """The part of `molframe.geometry.Superposition` the helpers read."""

    @property
    def rotation(self) -> Iterable[Iterable[float]]:
        """Row-major 3x3 rotation `R` in `R @ x + t`."""

    @property
    def translation(self) -> Iterable[float]:
        """Translation `t` in `R @ x + t`."""


def from_rotation_translation(
    rotation: Iterable[Iterable[float]], translation: Iterable[float]
) -> list[float]:
    """Return the 16 column-major floats for `x' = rotation @ x + translation`.

    `rotation` is a row-major 3x3 (nested sequence or NumPy array) and
    `translation` has three components. The result is what `Scene.place`
    expects, so `scene.place(molgfx.place.from_rotation_translation(R, t))`
    moves every atom to `R @ x + t`.
    """
    matrix = [[float(x) for x in row] for row in rotation]
    shift = [float(x) for x in translation]
    if len(matrix) != _DIM or any(len(row) != _DIM for row in matrix) or len(shift) != _DIM:
        message = "rotation must be 3x3 and translation must have 3 components"
        raise ValueError(message)
    rows = [[*matrix[i], shift[i]] for i in range(_DIM)] + [[0.0, 0.0, 0.0, 1.0]]
    return [rows[r][c] for c in range(4) for r in range(4)]


def from_superposition(superposition: _Superposition) -> list[float]:
    """Column-major placement for a molframe `Superposition` (`R @ x + t`)."""
    return from_rotation_translation(superposition.rotation, superposition.translation)


def apply(
    scene: Scene,
    superposition: _Superposition | tuple[Iterable[Iterable[float]], Iterable[float]],
    *,
    structure: StructureId | None = None,
) -> StructureId:
    """Place a structure with a molframe `Superposition` or a `(R, t)` pair."""
    if isinstance(superposition, tuple):
        rotation, translation = superposition
        matrix = from_rotation_translation(rotation, translation)
    else:
        matrix = from_superposition(superposition)
    return scene.place(matrix, structure=structure)
