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
"""

from .viewer import Viewer

__all__ = ["Viewer"]
