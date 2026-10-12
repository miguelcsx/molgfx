"""AnyWidget transport over one authoritative authoring Session."""

from __future__ import annotations

import json
import weakref
from pathlib import Path
from typing import TYPE_CHECKING, cast

import anywidget
import traitlets
from molframe import Structure

from molgfx._engine import Command, Scene, Session

from ._runtime import load_runtime

if TYPE_CHECKING:
    from collections.abc import Callable, Mapping, Sequence

    from molframe import Query

    from molgfx._engine import CommandResult, ScenePatch

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
        runtime = load_runtime()
        self.session = source if isinstance(source, Session) else Session(source)
        self._scene = self.session.scene
        if isinstance(source, Structure):
            self.session.execute(Command.auto())
        self._structures: dict[int, tuple[str, bytes]] = {}
        self._materialize(self.scene.browser_sources())
        ids, names, payloads, sources = self._structure_columns()
        volumes, hydrogen_bonds, surfaces = self._scientific_columns()
        frames, trajectories, playback = self._motion_columns()
        super().__init__(
            _esm=(_STATIC / "anywidget.js").read_text(encoding="utf-8"),
            _css=(_STATIC / "viewer.css").read_text(encoding="utf-8"),
            _runtime_js=runtime.glue,
            _runtime_wasm=runtime.wasm_gzip,
            _runtime_key=runtime.key,
            scene_spec=self.scene.to_json(),
            revision=self.scene.revision,
            structure_ids=ids,
            structure_names=names,
            structure_payloads=payloads,
            structure_sources=sources,
            volume_bindings=volumes,
            hydrogen_bond_bindings=hydrogen_bonds,
            molecular_surface_bindings=surfaces,
            coordinate_frames=frames,
            trajectory_bindings=trajectories,
            playback_bindings=playback,
            **kwargs,
        )
        self._closed = False
        self._subscription = weakref.WeakMethod(self._on_scene_patch)
        self.scene.subscribe(self._subscription)
        self._runtime_subscription = weakref.WeakMethod(self._on_runtime_sample)
        self.scene.subscribe_runtime(self._runtime_subscription)
        self.observe(self._on_sync_request, names="sync_request")
        self.observe(self._on_interaction, names="interaction_event")

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

    def focus(self, target: str | Query) -> CommandResult:
        """Focus a target with the Session's camera and context policy."""
        return self.execute(Command.focus(target))

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
        self._structures.clear()
        super().close()

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

    def _materialize(self, sources: list[tuple[int, str, bytes]]) -> None:
        declared = {identity for identity, _, _ in sources}
        for identity in list(self._structures.keys() - declared):
            del self._structures[identity]
        for identity, name, payload in sources:
            self._structures[identity] = (name, payload)

    def _structure_columns(self) -> tuple[list[int], list[str], list[bytes], list[int]]:
        ids = list(self._structures)
        names = [entry[0] for entry in self._structures.values()]
        owners: dict[bytes, int] = {}
        payloads: list[bytes] = []
        sources: list[int] = []
        for identity in ids:
            encoded = self._structures[identity][1]
            owner = owners.setdefault(encoded, identity)
            sources.append(owner)
            payloads.append(encoded if owner == identity else b"")
        return ids, names, payloads, sources

    def _resync_structures(self) -> None:
        self._materialize(self.scene.browser_sources())
        # All source columns precede the new spec in the same comm update.
        with self.hold_sync():
            (
                self.structure_ids,
                self.structure_names,
                self.structure_payloads,
                self.structure_sources,
            ) = self._structure_columns()
            (
                self.volume_bindings,
                self.hydrogen_bond_bindings,
                self.molecular_surface_bindings,
            ) = self._scientific_columns()
            self.coordinate_frames, self.trajectory_bindings, self.playback_bindings = (
                self._motion_columns()
            )
            self.revision = self.scene.revision
            self.scene_patch = ""
            self.scene_spec = self.scene.to_json()

    def _on_scene_patch(self, patch_json: str) -> None:
        if self._closed:
            return
        # The committed scene is authoritative even for empty or replayed patches.
        patch = cast("dict[str, object]", json.loads(patch_json))
        operations = cast("list[dict[str, object]]", patch["operations"])
        previous = self._binding_identity
        previous_motion = self._motion_identity
        self._scientific_columns()
        self._motion_columns()
        if (
            previous != self._binding_identity
            or previous_motion != self._motion_identity
            or any(
                operation["op"] in {"add_structure", "replace_structure", "remove_structure"}
                for operation in operations
            )
        ):
            self._resync_structures()
            return
        with self.hold_sync():
            self.revision = self.scene.revision
            self.scene_patch = patch_json
            self.patch_sequence += 1

    def _scientific_columns(
        self,
    ) -> tuple[
        list[tuple[str, bytes]],
        list[tuple[str, bytes, float | None, str]],
        list[tuple[str, str, bytes, bytes, bytes, bytes]],
    ]:
        volumes, hydrogen_bonds, surfaces = self.scene.browser_bindings()
        # Payload object identities are cached; style edits never compare large buffers.
        self._binding_identity = (
            tuple((identity, id(values)) for identity, values in volumes),
            tuple(
                (identity, id(rows), distance, metadata)
                for identity, rows, distance, metadata in hydrogen_bonds
            ),
            tuple(
                (identity, metadata, id(vertices), id(normals), id(faces), id(atoms))
                for identity, metadata, vertices, normals, faces, atoms in surfaces
            ),
        )
        return (
            [(str(identity), values) for identity, values in volumes],
            [
                (str(identity), rows, distance, metadata)
                for identity, rows, distance, metadata in hydrogen_bonds
            ],
            [
                (str(identity), metadata, vertices, normals, faces, atoms)
                for identity, metadata, vertices, normals, faces, atoms in surfaces
            ],
        )

    def _on_runtime_sample(self, event_json: str) -> None:
        if not self._closed:
            event: object = json.loads(event_json)
            if isinstance(event, dict):
                identity = str(event["structure"])
                frames, trajectories, playback = self._motion_columns()
                interval = next(interval for interval in playback if interval[0] == identity)
                with self.hold_sync():
                    self.coordinate_frames = frames
                    self.trajectory_bindings = trajectories
                    self.playback_bindings = playback
                    self.playback_pair = interval
                return
            identity, seconds = cast("tuple[str, float]", event)
            self.trajectory_sample = (identity, seconds)

    def _motion_columns(
        self,
    ) -> tuple[
        list[tuple[str, str, float, bytes]],
        list[tuple[str, str, str, float]],
        list[tuple[str, str, str, float]],
    ]:
        frames, intervals = self.scene.browser_motion_bindings()
        playback = self.scene.browser_playback_bindings()
        self._motion_identity = (
            tuple(
                (identity, index, seconds, id(positions))
                for identity, index, seconds, positions in frames
            ),
            tuple((identity, start, end) for identity, start, end, _ in intervals),
            tuple((identity, start, end) for identity, start, end, _ in playback),
        )
        return (
            [
                (str(identity), str(index), seconds, positions)
                for identity, index, seconds, positions in frames
            ],
            [
                (str(identity), str(start), str(end), seconds)
                for identity, start, end, seconds in intervals
            ],
            [
                (str(identity), str(start), str(end), seconds)
                for identity, start, end, seconds in playback
            ],
        )
