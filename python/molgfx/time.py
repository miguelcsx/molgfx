"""Typed physical-time vocabulary shared by trajectories and timelines."""

from enum import StrEnum


class TimeUnit(StrEnum):
    """A unit of physical time, spelled by its symbol."""

    ATTOSECOND = "as"
    FEMTOSECOND = "fs"
    PICOSECOND = "ps"
    NANOSECOND = "ns"
    MICROSECOND = "µs"
    MILLISECOND = "ms"
    SECOND = "s"


class PlaybackMode(StrEnum):
    """What a track does after reaching the end of its interval."""

    CLAMP = "clamp"
    LOOP = "loop"
    PING_PONG = "ping_pong"


__all__ = ["PlaybackMode", "TimeUnit"]
