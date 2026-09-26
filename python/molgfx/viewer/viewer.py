"""AnyWidget transport for MolGFX's direct browser WebGPU runtime."""

from functools import lru_cache
from hashlib import sha256
from pathlib import Path
import json
import weakref

import anywidget
import traitlets

_STATIC = Path(__file__).parent / "static"


@lru_cache(maxsize=1)
def _runtime():
    """The packaged browser runtime: its module text, binary and a content key.

    A notebook frontend loads the widget module from a blob URL, where the
    runtime beside it cannot be imported by a relative path, so both parts are
    sent as widget state. A checkout without a built runtime sends nothing and
    the page falls back to importing it beside the module.
    """
    glue = _STATIC / "molgfx_wasm.js"
    binary = _STATIC / "molgfx_wasm_bg.wasm"
    if not (glue.is_file() and binary.is_file()):
        return "", b"", ""
    code = glue.read_text(encoding="utf-8")
    data = binary.read_bytes()
    return code, data, sha256(data).hexdigest()[:16]


class Viewer(anywidget.AnyWidget):
    """A canvas backed by the same versioned scene contract as Rust and Python."""

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
    camera = traitlets.Dict().tag(sync=True)
    error = traitlets.Unicode().tag(sync=True)

    def __init__(self, scene, **kwargs):
        """Bind a scene and its compact BinaryCIF sources to the browser."""
        self._scene = scene
        self._structures = {}
        self._materialize(scene._browser_sources())
        code, data, key = _runtime()
        super().__init__(
            _runtime_js=code,
            _runtime_wasm=data,
            _runtime_key=key,
            scene_spec=scene.to_json(),
            structure_ids=list(self._structures),
            structure_names=[entry[0] for entry in self._structures.values()],
            structure_payloads=[entry[1] for entry in self._structures.values()],
            **kwargs,
        )
        self._subscription = weakref.WeakMethod(self._on_scene_patch)
        scene._subscribe(self._subscription)

    def _materialize(self, sources):
        """Record each structure's payload the first time it is announced."""
        for identity, name, payload in sources:
            if identity not in self._structures:
                self._structures[identity] = (name, bytes(payload))

    def _resync_structures(self):
        """Transfer every structure the scene now declares, each exactly once.

        The page rebuilds from ``scene_spec`` because binding a source is a
        resolve-time operation: the widget binds every transported structure and
        then resolves, so one spec replacement already carries the addition.
        """
        self._materialize(self._scene._browser_sources())
        if len(self._structures) == len(self.structure_ids):
            return
        self.structure_ids = list(self._structures)
        self.structure_names = [entry[0] for entry in self._structures.values()]
        self.structure_payloads = [entry[1] for entry in self._structures.values()]
        self.scene_spec = self._scene.to_json()

    def _on_scene_patch(self, patch_json):
        """Forward one already-committed semantic patch to the browser."""
        operations = json.loads(patch_json).get("operations", [])
        if any(operation.get("op") == "add_structure" for operation in operations):
            self._resync_structures()
            return
        self.scene_patch = patch_json
        self.patch_sequence += 1

    def apply(self, patch):
        """Apply one atomic patch locally and forward that exact patch once."""
        self._scene.apply(patch)
