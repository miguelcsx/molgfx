"""AnyWidget transport over one authoritative authoring Session."""

from __future__ import annotations

import json
import weakref
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import anywidget
import traitlets
from molframe import Structure

from molgfx._engine import Command, Scene, Session
from molgfx.time import PlaybackMode, TimeUnit

from ._runtime import load_runtime
from ._transport import (
    CHUNK_FRAMES,
    PlaybackTime,
    chunk_rows,
    decode_time,
    finite,
    to_seconds,
    wire_mode,
)

if TYPE_CHECKING:
    from collections.abc import Callable, Mapping, Sequence

    from molframe import Query

    from molgfx._engine import CommandResult, Playback, ScenePatch

    from ._transport import ChunkFrameRow

_STATIC = Path(__file__).parents[1] / "static"


class Viewer(anywidget.AnyWidget):
    """Render a Session, Scene or molframe Structure without application UI.

    Structures receive the Session's automatic representation command. Existing
    scenes and sessions retain their authored state. All authoring conveniences
    create typed commands; command validation and history belong to Session.
    """

    _esm = traitlets.Unicode().tag(sync=True)
    _css = traitlets.Unicode().tag(sync=True)
    _runtime_js = traitlets.Unicode("").tag(sync=True)
    _runtime_wasm = traitlets.Bytes(b"").tag(sync=True)
    _runtime_key = traitlets.Unicode("").tag(sync=True)
    scene_spec = traitlets.Unicode().tag(sync=True)
    scene_patch = traitlets.Unicode().tag(sync=True)
    patch_sequence = traitlets.Int(0).tag(sync=True)
    structure_ids = traitlets.List(traitlets.Int()).tag(sync=True)
    structure_names = traitlets.List(traitlets.Unicode()).tag(sync=True)
    structure_payloads = traitlets.List(traitlets.Bytes()).tag(sync=True)
    structure_sources = traitlets.List(traitlets.Int()).tag(sync=True)
    volume_bindings = traitlets.List(traitlets.Tuple(traitlets.Unicode(), traitlets.Bytes())).tag(
        sync=True
    )
    hydrogen_bond_bindings = traitlets.List(
        traitlets.Tuple(
            traitlets.Unicode(),
            traitlets.Bytes(),
            traitlets.Float(allow_none=True),
            traitlets.Unicode(),
        )
    ).tag(sync=True)
    molecular_surface_bindings = traitlets.List(
        traitlets.Tuple(
            traitlets.Unicode(),
            traitlets.Unicode(),
            traitlets.Bytes(),
            traitlets.Bytes(),
            traitlets.Bytes(),
            traitlets.Bytes(),
        )
    ).tag(sync=True)
    coordinate_frames = traitlets.List(
        traitlets.Tuple(
            traitlets.Unicode(), traitlets.Unicode(), traitlets.Float(), traitlets.Bytes()
        )
    ).tag(sync=True)
    trajectory_bindings = traitlets.List(
        traitlets.Tuple(
            traitlets.Unicode(), traitlets.Unicode(), traitlets.Unicode(), traitlets.Float()
        )
    ).tag(sync=True)
    trajectory_sample = traitlets.Tuple(
        traitlets.Unicode(), traitlets.Float(), default_value=("", 0.0)
    ).tag(sync=True)
    playback_bindings = traitlets.List(
        traitlets.Tuple(
            traitlets.Unicode(), traitlets.Unicode(), traitlets.Unicode(), traitlets.Float()
        )
    ).tag(sync=True)
    playback_pair = traitlets.Tuple(
        traitlets.Unicode(),
        traitlets.Unicode(),
        traitlets.Unicode(),
        traitlets.Float(),
        default_value=("", "", "", 0.0),
    ).tag(sync=True)
    trajectory_chunk = traitlets.Tuple(
        traitlets.Int(),
        traitlets.Unicode(),
        traitlets.List(
            traitlets.Tuple(
                traitlets.Unicode(), traitlets.Unicode(), traitlets.Float(), traitlets.Bytes()
            )
        ),
        default_value=(0, "", []),
    ).tag(sync=True)
    pick = traitlets.Dict().tag(sync=True)
    camera = traitlets.Dict().tag(sync=True)
    error = traitlets.Unicode().tag(sync=True)
    interaction_event = traitlets.Dict().tag(sync=True)
    revision = traitlets.Int(0).tag(sync=True)
    sync_request = traitlets.Int(0).tag(sync=True)

    def __init__(self, source: Session | Scene | Structure, **kwargs: object) -> None:
        """Bind one live session and its BinaryCIF sources to the browser."""
        self._closed = True
        self._subscription: weakref.WeakMethod[Callable[[str], None]] | None = None
        self._runtime_subscription: weakref.WeakMethod[Callable[[str], None]] | None = None
        self._last_clear_event: str | None = None
        self._time: PlaybackTime | None = None
        self._raw_time: tuple[object, object] | None = None
        self._streams: dict[str, tuple[Sequence[object], list[float]]] = {}
        self._chunk_serial = 0
        runtime = load_runtime()
        self.session = source if isinstance(source, Session) else Session(source)
        self._scene = self.session.scene
        if isinstance(source, Structure):
            self.session.execute(Command.auto())
        transport = self.scene.browser_transport()
        self._binding_fingerprint = transport.binding_fingerprint
        self._motion_fingerprint = transport.motion_fingerprint
        super().__init__(
            _esm=(_STATIC / "anywidget.js").read_text(encoding="utf-8"),
            _css=(_STATIC / "viewer.css").read_text(encoding="utf-8"),
            _runtime_js=runtime.glue,
            _runtime_wasm=runtime.wasm_gzip,
            _runtime_key=runtime.key,
            scene_spec=self.scene.to_json(),
            revision=self.scene.revision,
            structure_ids=transport.structure_ids,
            structure_names=transport.structure_names,
            structure_payloads=transport.structure_payloads,
            structure_sources=transport.structure_sources,
            volume_bindings=transport.volumes,
            hydrogen_bond_bindings=transport.hydrogen_bonds,
            molecular_surface_bindings=transport.surfaces,
            coordinate_frames=transport.frames,
            trajectory_bindings=transport.trajectories,
            playback_bindings=transport.playback,
            **kwargs,
        )
        self._closed = False
        self._subscription = weakref.WeakMethod(self._on_scene_patch)
        self.scene.subscribe(self._subscription)
        self._runtime_subscription = weakref.WeakMethod(self._on_runtime_sample)
        self.scene.subscribe_runtime(self._runtime_subscription)
        self.observe(self._on_sync_request, names="sync_request")
        self.observe(self._on_interaction, names="interaction_event")
        # anywidget's custom-message API is untyped; Any states that once, here.
        widget: Any = self
        widget.on_msg(self._on_message)

    @property
    def scene(self) -> Scene:
        """The live scene owned by the exposed Session."""
        return self._scene

    def execute(self, program: str | Command | Sequence[Command]) -> CommandResult:
        """Execute through Session, preserving its atomicity and typed errors."""
        if self._closed:
            message = "cannot execute commands through a closed Viewer"
            raise RuntimeError(message)
        return self.session.execute(program)

    def show(self, form: str, target: str | Query = "all", **options: object) -> CommandResult:
        """Show a representation using the Session's typed show command."""
        return self.execute(Command.show(form, target, **options))

    def hide(self, layer: str) -> CommandResult:
        """Hide a named Session layer."""
        return self.execute(Command.hide(layer))

    def color(
        self, color: str, target: str | Query = "all", *, structure: str | None = None
    ) -> CommandResult:
        """Apply the Session's color command to a target."""
        return self.execute(Command.color(color, target, structure=structure))

    def focus(self, target: str | Query | None = None) -> None:
        """Focus a target with the Session's camera and context policy.

        Without a target this is the widget's own keyboard-focus request, which
        the base class defines with no arguments. The base signature returns
        nothing; run ``Command.focus`` through ``execute`` for the result.
        """
        if target is None:
            super().focus()
        else:
            self.execute(Command.focus(target))

    def select(self, target: str | Query | None) -> CommandResult:
        """Set or clear the selected overlay as one undoable Session command."""
        return self.execute(Command.set_selection(target))

    def apply(self, patch: ScenePatch) -> None:
        """Apply a semantic patch and publish it through the scene subscription."""
        if self._closed:
            message = "cannot apply a patch through a closed Viewer"
            raise RuntimeError(message)
        self.scene.apply(patch)

    def close(self) -> None:
        """Deterministically detach transport callbacks before closing the comm."""
        if self._closed:
            return
        self._closed = True
        if self._subscription is not None:
            self.scene.unsubscribe(self._subscription)
            self._subscription = None
        if self._runtime_subscription is not None:
            self.scene.unsubscribe_runtime(self._runtime_subscription)
            self._runtime_subscription = None
        self.unobserve(self._on_sync_request, names="sync_request")
        self.unobserve(self._on_interaction, names="interaction_event")
        widget: Any = self
        widget.on_msg(self._on_message, remove=True)
        self._streams.clear()
        super().close()

    @property
    def time(self) -> PlaybackTime | None:
        """The browser transport's latest position, or ``None`` before the first tick."""
        if self._time is None and self._raw_time is not None:
            self._time = decode_time(*self._raw_time)
        return self._time

    def play(self) -> None:
        """Start the browser transport."""
        self._command({"type": "play"})

    def pause(self) -> None:
        """Pause the browser transport."""
        self._command({"type": "pause"})

    def seek(self, seconds: float) -> None:
        """Move the browser transport to ``seconds``."""
        self._command({"type": "seek", "seconds": finite(seconds, "seconds")})

    def set_rate(self, rate: float) -> None:
        """Set the playback rate; the browser transport validates its range."""
        self._command({"type": "set_rate", "rate": finite(rate, "rate")})

    def set_loop(self, mode: PlaybackMode) -> None:
        """Set what the transport does at the end; only a ``PlaybackMode`` is accepted."""
        self._command({"type": "set_loop", "mode": wire_mode(mode)})

    def stream_trajectory(self, playback: Playback, *, unit: TimeUnit = TimeUnit.SECOND) -> None:
        """Serve ``playback``'s frames to the browser in chunks and push the first one.

        ``unit`` is the unit the playback's frame times are expressed in. The
        browser pulls later chunks with ``chunk_request`` messages.
        """
        if self._closed:
            message = "cannot stream a trajectory through a closed Viewer"
            raise RuntimeError(message)
        structure = str(playback.structure.value)
        positions = cast("Sequence[object]", playback.positions)
        self._streams[structure] = (positions, to_seconds(playback.times, unit))
        self._reply_chunk(structure, float("-inf"), CHUNK_FRAMES)

    def sample_trajectory(self, seconds: float) -> None:
        """Ask every streamed structure for the chunk that brackets ``seconds``."""
        if self._closed:
            message = "cannot sample a trajectory through a closed Viewer"
            raise RuntimeError(message)
        at = finite(seconds, "seconds")
        for structure in self._streams:
            self._reply_chunk(structure, at, CHUNK_FRAMES)

    def _command(self, message: dict[str, object]) -> None:
        if self._closed:
            text = "cannot control playback through a closed Viewer"
            raise RuntimeError(text)
        widget: Any = self
        widget.send(message)

    def _on_message(self, _widget: object, content: object, _buffers: object) -> None:
        if self._closed or not isinstance(content, dict):
            return
        message = cast("dict[str, object]", content)
        kind = message.get("type")
        if kind == "timechange":
            self._raw_time = (message.get("seconds"), message.get("readouts"))
            self._time = None
        elif kind == "chunk_request":
            structure = message.get("structure")
            origin = message.get("from_seconds")
            count = message.get("count")
            if (
                isinstance(structure, str)
                and isinstance(origin, (int, float))
                and isinstance(count, int)
                and not isinstance(count, bool)
                and structure in self._streams
            ):
                self._reply_chunk(structure, float(origin), count)

    def _reply_chunk(self, structure: str, from_seconds: float, count: int) -> None:
        positions, seconds = self._streams[structure]
        self._chunk_serial += 1
        rows: list[ChunkFrameRow] = chunk_rows(
            f"{structure}:{self._chunk_serial}", positions, seconds, from_seconds, count
        )
        self.trajectory_chunk = (self._chunk_serial, structure, rows)

    def _on_sync_request(self, _change: Mapping[str, object]) -> None:
        if not self._closed:
            self._resync_structures()

    def _on_interaction(self, change: Mapping[str, object]) -> None:
        if self._closed:
            return
        value = change["new"]
        if not isinstance(value, dict):
            return
        event = cast("dict[str, object]", value)
        identity = event.get("eventId")
        if event.get("kind") != "clear" or not isinstance(identity, str):
            return
        if identity == self._last_clear_event:
            return
        self._last_clear_event = identity
        self.select(None)

    def _resync_structures(self) -> None:
        transport = self.scene.browser_transport()
        self._binding_fingerprint = transport.binding_fingerprint
        self._motion_fingerprint = transport.motion_fingerprint
        # All source columns precede the new spec in the same comm update.
        with self.hold_sync():
            self.structure_ids = transport.structure_ids
            self.structure_names = transport.structure_names
            self.structure_payloads = transport.structure_payloads
            self.structure_sources = transport.structure_sources
            self.volume_bindings = transport.volumes
            self.hydrogen_bond_bindings = transport.hydrogen_bonds
            self.molecular_surface_bindings = transport.surfaces
            self.coordinate_frames = transport.frames
            self.trajectory_bindings = transport.trajectories
            self.playback_bindings = transport.playback
            self.revision = self.scene.revision
            self.scene_patch = ""
            self.scene_spec = self.scene.to_json()

    def _on_scene_patch(self, patch_json: str) -> None:
        if self._closed:
            return
        # The committed scene is authoritative even for empty or replayed patches.
        transport = self.scene.browser_transport()
        if (
            transport.binding_fingerprint != self._binding_fingerprint
            or transport.motion_fingerprint != self._motion_fingerprint
            or Scene.patch_changes_structures(patch_json)
        ):
            self._resync_structures()
            return
        with self.hold_sync():
            self.revision = self.scene.revision
            self.scene_patch = patch_json
            self.patch_sequence += 1

    def _on_runtime_sample(self, event_json: str) -> None:
        if not self._closed:
            event: object = json.loads(event_json)
            if isinstance(event, dict):
                pair = cast("dict[str, object]", event)
                transport = self.scene.browser_transport()
                self._motion_fingerprint = transport.motion_fingerprint
                interval = transport.playback_for(str(pair["structure"]))
                with self.hold_sync():
                    self.coordinate_frames = transport.frames
                    self.trajectory_bindings = transport.trajectories
                    self.playback_bindings = transport.playback
                    self.playback_pair = interval
                return
            identity, seconds = cast("tuple[str, float]", event)
            self.trajectory_sample = (identity, seconds)
