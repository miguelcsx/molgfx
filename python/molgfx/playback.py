"""Play an N-frame trajectory through a scene.

A scene holds one resident frame *pair* plus a sample time. Stepping through a
longer trajectory therefore means moving that pair as time advances. `attach`
does this with `Scene.set_trajectory_pair`, which needs no `trajectory.bind`
descriptor and no rebinding::

    traj = molframe.trajectory.read("run.xtc", topology="top.pdb")
    play = molgfx.playback.attach_trajectory(scene, traj)
    images = play.render(molgfx.Renderer(), times=[0.0, 0.5, 1.0], size=(640, 480))
"""

from bisect import bisect_right
from collections.abc import Iterable
from dataclasses import dataclass, field
from itertools import pairwise
from typing import Protocol

from ._engine import Image, Renderer, Scene, StructureId, trajectory

__all__ = ["Playback", "attach", "attach_trajectory"]


class _Frames(Protocol):
    """A `(frames, atoms, 3)` coordinate array such as `numpy.ndarray`."""

    def __len__(self) -> int:
        """Return the number of frames."""

    def __getitem__(self, index: int, /) -> Iterable[Iterable[float]]:
        """Return one frame as atoms of three coordinates."""


class _Trajectory(Protocol):
    """The part of `molframe.trajectory.Trajectory` that playback reads."""

    @property
    def positions(self) -> _Frames:
        """Coordinates indexed by frame, atom and axis."""

    @property
    def times(self) -> Iterable[float]:
        """Frame times in ascending order."""


_MIN_FRAMES = 2


@dataclass(slots=True)
class Playback:
    """A scene bound to explicit frames; `seek` moves the resident pair."""

    scene: Scene
    structure: StructureId
    positions: _Frames
    times: tuple[float, ...]
    _segment: int = field(default=-1, init=False, repr=False)

    def seek(self, time: float) -> None:
        """Make the scene show the interpolated state at `time`.

        `time` is clamped to the first and last frame time.
        """
        last = len(self.times) - 1
        clamped = min(max(float(time), float(self.times[0])), float(self.times[last]))
        index = min(max(bisect_right(self.times, clamped) - 1, 0), last - 1)
        start = trajectory.frame(index, float(self.times[index]), _rows(self.positions[index]))
        end = trajectory.frame(
            index + 1, float(self.times[index + 1]), _rows(self.positions[index + 1])
        )
        self.scene.set_trajectory_pair(
            structure=self.structure, start=start, end=end, sample_time=clamped
        )
        self._segment = index

    def render(
        self, renderer: Renderer, *, times: Iterable[float], size: tuple[int, int]
    ) -> list[Image]:
        """Render one image per entry of `times`, seeking before each."""
        images: list[Image] = []
        for time in times:
            self.seek(time)
            images.append(renderer.render_image(self.scene, size=size))
        return images


def _rows(frame: Iterable[Iterable[float]]) -> list[list[float]]:
    return [[float(axis) for axis in atom] for atom in frame]


def attach(
    scene: Scene,
    positions: _Frames,
    times: Iterable[float],
    *,
    structure: StructureId | None = None,
) -> Playback:
    """Bind frames to a structure and return a `Playback` positioned at frame 0.

    `positions` is `(frames, atoms, 3)` in the structure's atom order and
    `times` has one strictly ascending entry per frame. At least two frames
    are needed.
    """
    stamps = tuple(float(time) for time in times)
    if len(positions) < _MIN_FRAMES or len(positions) != len(stamps):
        message = "playback needs at least two frames and one time per frame"
        raise ValueError(message)
    if any(later <= earlier for earlier, later in pairwise(stamps)):
        message = "times must be strictly ascending"
        raise ValueError(message)
    play = Playback(scene, structure or scene.structure_id, positions, stamps)
    play.seek(stamps[0])
    return play


def attach_trajectory(
    scene: Scene, source: _Trajectory, *, structure: StructureId | None = None
) -> Playback:
    """`attach` for a molframe `Trajectory` (its `positions` and `times`)."""
    return attach(scene, source.positions, source.times, structure=structure)
