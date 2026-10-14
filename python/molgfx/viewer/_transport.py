"""Playback messages, readouts and trajectory chunks for the notebook viewer."""

from __future__ import annotations

import math
from array import array
from bisect import bisect_right
from collections.abc import Buffer, Iterable, Sequence
from dataclasses import dataclass
from hashlib import blake2b
from itertools import chain
from typing import Final, cast

from molgfx.time import PlaybackMode, TimeUnit

#: Frames per chunk; the browser asks for this many at a time (``CHUNK_FRAMES`` in the web package).
CHUNK_FRAMES: Final = 8
#: Most frames one reply may carry, whatever the browser asks for.
MAX_CHUNK_FRAMES: Final = 4 * CHUNK_FRAMES

#: The browser spells ping-pong without an underscore.
_WIRE_MODE: Final = {
    PlaybackMode.CLAMP: "clamp",
    PlaybackMode.LOOP: "loop",
    PlaybackMode.PING_PONG: "pingpong",
}
#: The browser spells microseconds ``us``.
_UNITS: Final = {"us": TimeUnit.MICROSECOND}

_SECONDS_PER: Final = {
    TimeUnit.ATTOSECOND: 1e-18,
    TimeUnit.FEMTOSECOND: 1e-15,
    TimeUnit.PICOSECOND: 1e-12,
    TimeUnit.NANOSECOND: 1e-9,
    TimeUnit.MICROSECOND: 1e-6,
    TimeUnit.MILLISECOND: 1e-3,
    TimeUnit.SECOND: 1.0,
}

type ChunkFrameRow = tuple[str, str, float, bytes]


@dataclass(frozen=True, slots=True)
class Readout:
    """One track's physical time, in its own display unit."""

    track: int
    value: float
    unit: TimeUnit
    phase: float


@dataclass(frozen=True, slots=True)
class PlaybackTime:
    """The browser transport's most recent position."""

    seconds: float
    readouts: tuple[Readout, ...]


def to_seconds(times: Iterable[float], unit: TimeUnit) -> list[float]:
    """Convert frame times in ``unit`` to the seconds the browser transport speaks."""
    factor = _SECONDS_PER[TimeUnit(unit)]
    return [float(time) * factor for time in times]


def wire_mode(mode: object) -> str:
    """Return the browser spelling of a loop mode, refusing anything but the enum."""
    if not isinstance(mode, PlaybackMode):
        message = f"set_loop requires a PlaybackMode, got {type(mode).__name__}"
        raise TypeError(message)
    return _WIRE_MODE[mode]


def finite(value: float, name: str) -> float:
    """Return ``value`` as a float, refusing NaN and infinities."""
    number = float(value)
    if not math.isfinite(number):
        message = f"{name} must be finite"
        raise ValueError(message)
    return number


def decode_time(seconds: object, readouts: object) -> PlaybackTime | None:
    """Decode a ``timechange`` message; ``None`` when it is malformed."""
    stride = 4
    if (
        not isinstance(seconds, (int, float))
        or not isinstance(readouts, list)
        or len(cast("list[object]", readouts)) % stride
    ):
        return None
    cells = cast("list[object]", readouts)
    rows: list[Readout] = []
    for base in range(0, len(cells), stride):
        track, value, unit, phase = cells[base : base + stride]
        if not (
            isinstance(track, (int, float))
            and isinstance(value, (int, float))
            and isinstance(unit, str)
            and isinstance(phase, (int, float))
        ):
            return None
        display = _UNITS.get(unit)
        if display is None:
            try:
                display = TimeUnit(unit)
            except ValueError:
                return None
        rows.append(Readout(int(track), float(value), display, float(phase)))
    return PlaybackTime(float(seconds), tuple(rows))


def frame_bytes(frame: object) -> bytes:
    """Pack one ``(atoms, 3)`` frame as float32 triples in native (little-endian) order."""
    if isinstance(frame, Buffer):
        view = memoryview(frame)
        if view.format == "f" and view.c_contiguous:
            return view.tobytes()
    if not isinstance(frame, Iterable):
        message = "trajectory frames must be (atoms, 3) sequences or buffers"
        raise TypeError(message)
    rows = cast("Iterable[Iterable[float]]", frame)
    return array("f", chain.from_iterable(rows)).tobytes()


def frame_identity(stream: str, index: int) -> str:
    """Name one streamed frame by a u64 unique per stream (structure and serial) and index."""
    digest = blake2b(f"{stream}:{index}".encode(), digest_size=8).digest()
    return str(int.from_bytes(digest, "little"))


def chunk_rows(
    stream: str,
    positions: Sequence[object],
    seconds: Sequence[float],
    from_seconds: float,
    count: int,
) -> list[ChunkFrameRow]:
    """Return up to ``count`` frames starting at the last one at or before ``from_seconds``.

    A request before the first frame starts at index 0. The reply is capped at
    ``MAX_CHUNK_FRAMES`` and ends with the trajectory.
    """
    if not seconds:
        return []
    start = max(bisect_right(seconds, from_seconds) - 1, 0)
    stop = min(start + max(count, 0), start + MAX_CHUNK_FRAMES, len(seconds))
    return [
        (
            frame_identity(stream, index),
            str(index),
            float(seconds[index]),
            frame_bytes(positions[index]),
        )
        for index in range(start, stop)
    ]
