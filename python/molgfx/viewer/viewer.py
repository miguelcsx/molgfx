"""AnyWidget transport for MolGFX's direct browser WebGPU runtime."""

from pathlib import Path
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
        sources = scene._browser_sources()
        super().__init__(
            scene_spec=scene.to_json(),
            structure_ids=list(map(lambda source: source[0], sources)),
            structure_names=list(map(lambda source: source[1], sources)),
            structure_payloads=list(map(lambda source: source[2], sources)),
            **kwargs,
        )
        self._scene = scene
        self._subscription = weakref.WeakMethod(self._on_scene_patch)
        scene._subscribe(self._subscription)

    def _on_scene_patch(self, patch_json):
        """Forward one already-committed semantic patch to the browser."""
        self.scene_patch = patch_json
        self.patch_sequence += 1

    def apply(self, patch):
        """Apply one atomic patch locally and forward that exact patch once."""
        self._scene.apply(patch)
