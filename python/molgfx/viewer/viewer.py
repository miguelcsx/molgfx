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
        self._last_clear_event: str | None = None
        runtime = load_runtime()
        self.session = source if isinstance(source, Session) else Session(source)
        self._scene = self.session.scene
        if isinstance(source, Structure):
            self.session.execute(Command.auto())
        self._structures: dict[int, tuple[str, bytes]] = {}
        self._materialize(self.scene.browser_sources())
        ids, names, payloads, sources = self._structure_columns()
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
            **kwargs,
        )
        self._closed = False
        self._subscription = weakref.WeakMethod(self._on_scene_patch)
        self.scene.subscribe(self._subscription)
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
            self.revision = self.scene.revision
            self.scene_patch = ""
            self.scene_spec = self.scene.to_json()

    def _on_scene_patch(self, patch_json: str) -> None:
        if self._closed:
            return
        # The committed scene is authoritative even for empty or replayed patches.
        patch = cast("dict[str, object]", json.loads(patch_json))
        operations = cast("list[dict[str, object]]", patch["operations"])
        if any(
            operation["op"] in {"add_structure", "replace_structure", "remove_structure"}
            for operation in operations
        ):
            self._resync_structures()
            return
        with self.hold_sync():
            self.revision = self.scene.revision
            self.scene_patch = patch_json
            self.patch_sequence += 1
