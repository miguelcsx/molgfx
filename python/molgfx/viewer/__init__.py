"""The Jupyter widget: a notebook viewport over the native engine.

The widget is glue, not computation — pointer events cross as data, the
pointer-to-camera step and the render run in Rust, and the kernel ships the
finished frame to the browser. Interaction renders at a quarter of the area
with the light profile; releasing the pointer settles into one full-quality
frame. It is the advanced interaction mode from day one, not a stepping
stone:

```python
import molframe, molgfx as mg
from molgfx.viewer import Viewer

scene = mg.Scene.from_structure_shared(molframe.read("1abc.cif"))
scene.represent(scene.select(mg.Select.parse("polymer")), mg.Representation.cartoon())

Viewer(scene)
```

The widget rides on ``anywidget`` (installed with ``pip install
"molgfx[jupyter]"``) and renders on demand: between interactions the engine
does nothing.
"""

from .viewer import Viewer

__all__ = ["Viewer"]