"""AnyWidget transport adapter for the viewer host built by molgfx-wasm."""

import json
import weakref
from pathlib import Path

import anywidget
import traitlets

from ._runtime import load_runtime

_STATIC = Path(__file__).parent / "static"

if not (_STATIC / "widget.js").is_file():
    # A wheel always carries the runtime; only a source checkout can lack it.
    raise ImportError(
        "the MolGFX browser runtime is not built; from the repository root run "
        "`node crates/molgfx-wasm/js/scripts/build.mjs --out python/molgfx/viewer/static`"
    )


class Viewer(anywidget.AnyWidget):
    """A scene transport backed by the shared molgfx-wasm viewer host."""

    _esm = _STATIC / "widget.js"
    _css = _STATIC / "widget.css"

    _runtime_js = traitlets.Unicode("").tag(sync=True)
    _runtime_wasm = traitlets.Bytes(b"").tag(sync=True)
    _runtime_key = traitlets.Unicode("").tag(sync=True)

    scene_spec = traitlets.Unicode().tag(sync=True)
    scene_patch = traitlets.Unicode().tag(sync=True)
    patch_sequence = traitlets.Int(0).tag(sync=True)
    structure_ids = traitlets.List(traitlets.Int()).tag(sync=True)
    structure_names = traitlets.List(traitlets.Unicode()).tag(sync=True)
    structure_payloads = traitlets.List(traitlets.Bytes()).tag(sync=True)
    pick = traitlets.Dict().tag(sync=True)
    selection = traitlets.Unicode().tag(sync=True)
    interaction = traitlets.Dict().tag(sync=True)
    camera = traitlets.Dict().tag(sync=True)
    error = traitlets.Unicode().tag(sync=True)
    interaction_event = traitlets.Dict().tag(sync=True)
    revision = traitlets.Int(0).tag(sync=True)
    sync_request = traitlets.Int(0).tag(sync=True)

    def __init__(self, scene, **kwargs):
        """Bind a scene and its compact BinaryCIF sources to the browser."""
        self._scene = scene
        self._structures = {}
        self._materialize(scene._browser_sources())
        runtime = load_runtime()
        ids, names, payloads = self._structure_columns()
        scene_spec = scene.to_json()
        super().__init__(
            _runtime_js=runtime.glue,
            _runtime_wasm=runtime.wasm_gzip,
            _runtime_key=runtime.key,
            scene_spec=scene_spec,
            revision=json.loads(scene_spec).get("revision", 0),
            structure_ids=ids,
            structure_names=names,
            structure_payloads=payloads,
            **kwargs,
        )
        self._subscription = weakref.WeakMethod(self._on_scene_patch)
        scene._subscribe(self._subscription)
        self.observe(self._on_sync_request, names="sync_request")

    def _on_sync_request(self, _change):
        """Send the current specification to a view that fell behind."""
        self._resync_structures()

    def _materialize(self, sources):
        """Record each structure's payload the first time it is announced."""
        for identity, name, payload in sources:
            encoded = bytes(payload)
            if self._structures.get(identity) != (name, encoded):
                self._structures[identity] = (name, encoded)

    def _release_unreferenced(self, sources):
        """Drop materialized payloads the scene no longer declares.

        Structure replacement and removal leave bytes in this cache that no
        current structure references; keeping them would pin megabytes of
        coordinate payload for the widget's lifetime. A re-announced identity
        whose encoding changed is refreshed by ``_materialize`` instead.
        """
        declared = {identity for identity, _, _ in sources}
        for identity in [key for key in self._structures if key not in declared]:
            del self._structures[identity]

    def _structure_columns(self):
        """The materialized structures as the three parallel transport columns."""
        ids = list(self._structures)
        names = [entry[0] for entry in self._structures.values()]
        payloads = [entry[1] for entry in self._structures.values()]
        return ids, names, payloads

    def _resync_structures(self):
        """Transfer every structure the scene now declares, each exactly once.

        The page rebuilds from scene_spec because binding a source is a
        resolve-time operation: the widget binds every transported structure and
        then resolves, so one spec replacement already carries the addition.
        """
        sources = self._scene._browser_sources()
        self._release_unreferenced(sources)
        self._materialize(sources)
        self.structure_ids, self.structure_names, self.structure_payloads = (
            self._structure_columns()
        )
        self.scene_spec = self._scene.to_json()

    def _on_scene_patch(self, patch_json):
        """Forward one already-committed semantic patch to the browser."""
        patch = json.loads(patch_json)
        operations = patch.get("operations", [])
        if operations:
            self.revision = patch.get("base_revision", 0) + 1
        # A structure change is not patchable onto a resolved page: binding is
        # resolve-time, so the page rebuilds from a full spec replacement. That
        # replacement may also drop payloads for removed or replaced sources.
        if any(
            operation.get("op")
            in ("add_structure", "replace_structure", "remove_structure")
            for operation in operations
        ):
            self._resync_structures()
            return
        self.scene_patch = patch_json
        self.patch_sequence += 1

    def apply(self, patch):
        """Apply one atomic patch locally and forward that exact patch once."""
        self._scene.apply(patch)
