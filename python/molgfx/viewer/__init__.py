"""Notebook canvas transport over the same browser frontend and Session commands.

``Viewer(structure)`` authors automatic representations through Session;
``Viewer(scene)`` and ``Viewer(session)`` preserve existing authoring state.
The normal AnyWidget dependency renders locally with packaged WebGPU assets.
"""

from .viewer import Viewer

__all__ = ["Viewer"]
