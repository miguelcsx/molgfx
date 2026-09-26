"""The Jupyter widget: direct WebGPU over the shared WASM scene contract.

The kernel sends a canonical ``SceneSpec`` and compact BinaryCIF source data.
Later edits are revision-checked ``ScenePatch`` messages; rendered pixels stay
in the browser:

```python
import molframe, molgfx as mg
from molgfx.viewer import Viewer

scene = mg.Scene(molframe.read("1abc.cif"))
scene.add(mg.rep.cartoon(target=mg.sel.protein()))

Viewer(scene)
```

The widget rides on ``anywidget`` (installed with ``pip install
"molgfx[jupyter]"``) and renders on demand with the packaged MolGFX WASM
runtime.

``Workbench`` adds a command line to the same canvas: commands run against a
``molgfx.Session`` in the kernel and reach the page as ordinary patches.
"""

from .viewer import Viewer
from .workbench import Workbench

__all__ = ["Viewer", "Workbench"]
