"""AnyWidget transport for MolGFX's direct browser WebGPU runtime."""

from pathlib import Path
import json
import weakref

import anywidget
import traitlets


class Viewer(anywidget.AnyWidget):
    """A canvas backed by the same versioned scene contract as Rust and Python."""

    _esm = Path(__file__).parent / "static" / "widget.js"
    _css = Path(__file__).parent / "static" / "widget.css"

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
        super().__init__(
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
