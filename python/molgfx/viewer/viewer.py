"""The anywidget skin over the native interaction path.

This module is glue. Every measurable computation — camera mathematics,
rendering, readback, compression — runs in Rust behind ``molgfx.Engine``;
what lives here is event forwarding (browser state dict → ``InputEvent``),
one profile switch per interaction phase, and the base64 transport of the
frame the kernel already rendered. The ``python_surface`` gate enforces the
shape: no loops, no comprehensions, no numpy in this subpackage.
"""

import base64
import json

import anywidget
import traitlets

import molgfx as mg

#: Pointer-button identifier in the browser ``pointerdown``/``pointerup``
#: events, mapped to the engine's abstract buttons once, here, as data.
_BUTTONS = {
    0: mg.core.Button.Left,
    1: mg.core.Button.Middle,
    2: mg.core.Button.Right,
}

_ESM = """
/**
 * The viewport skin: pointer events become coalesced kernel requests, and
 * the kernel's finished frame is blitted. Camera and pixel work all happen
 * in Rust behind the kernel; this module only forwards and displays.
 */
function render({ model, el }) {
  el.classList.add("molgfx-viewer");
  const img = document.createElement("img");
  img.className = "molgfx-frame";
  img.draggable = false;
  el.appendChild(img);

  model.on("change:frame_png", () => {
    const png = model.get("frame_png");
    if (png) img.src = "data:image/png;base64," + png;
  });

  // Coalescing: the newest pointer state wins, at most one request is in
  // flight, and the next one ships on the animation frame after a reply.
  let latest = null;
  let dirty = false;
  let awaiting = false;
  let scheduled = false;
  let guard = 0;

  const flush = () => {
    scheduled = false;
    if (!dirty || awaiting) return;
    dirty = false;
    awaiting = true;
    guard = setTimeout(() => { awaiting = false; }, 5000);
    model.set("pointer_state", JSON.stringify(latest));
    model.save_changes();
  };
  const schedule = () => {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(flush);
  };
  model.on("change:frame_png", () => {
    awaiting = false;
    clearTimeout(guard);
    schedule();
  });

  const position = (event) => {
    const rect = img.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  };

  let dragging = false;
  img.addEventListener("pointerdown", (event) => {
    dragging = true;
    img.setPointerCapture(event.pointerId);
    latest = { kind: "button", button: event.button, pressed: true, dragging: true, ...position(event) };
    dirty = true;
    schedule();
    event.preventDefault();
  });
  img.addEventListener("pointermove", (event) => {
    if (!dragging) return;
    latest = { kind: "move", dragging: true, ...position(event) };
    dirty = true;
    schedule();
  });
  img.addEventListener("pointerup", (event) => {
    dragging = false;
    latest = { kind: "button", button: event.button, pressed: false, dragging: false, ...position(event) };
    dirty = true;
    schedule();
  });
  img.addEventListener("wheel", (event) => {
    latest = { kind: "scroll", delta: -event.deltaY * 0.01, dragging: true };
    dirty = true;
    schedule();
    event.preventDefault();
  }, { passive: false });

  return () => {};
}

export default { render };
"""

_CSS = """
.molgfx-viewer {
  display: inline-block;
  line-height: 0;
}

.molgfx-frame {
  max-width: 100%;
  touch-action: none;
}
"""


class Viewer(anywidget.AnyWidget):
    """A live molecular viewport for notebooks.

    Glue over the native engine: the browser sends coalesced pointer state,
    the kernel maps it to ``InputEvent`` values and lets the Rust camera
    controller update the camera, renders at a quarter of the area with the
    light profile while the pointer is down, and settles into one
    full-resolution, full-profile frame on release. Between interactions the
    engine renders nothing.

    ```python
    import molframe, molgfx as mg
    from molgfx.viewer import Viewer

    scene = mg.Scene.from_structure_shared(molframe.read("1abc.cif"))
    scene.represent(scene.select(mg.Select.parse("polymer")), mg.Representation.cartoon())

    Viewer(scene, width=960, height=540)
    ```

    ``scene`` is required; ``camera``, ``profile`` and ``controller`` default
    to a camera framing the scene, the profile the viewer was given, and an
    arcball controller. The interaction frame budget is the engine's: the
    scene stays GPU-resident, so a camera-only frame costs no re-upload.
    """

    _esm = _ESM
    _css = _CSS

    frame_png = traitlets.Unicode("").tag(sync=True)
    pointer_state = traitlets.Unicode("").tag(sync=True)
    width = traitlets.Int(960).tag(sync=True)
    height = traitlets.Int(540).tag(sync=True)

    def __init__(self, scene, camera=None, profile=None, controller=None, **kwargs):
        self._scene = scene
        self._profile = profile or mg.RenderProfile.inspection()
        self._controller = controller or mg.core.ArcballController()
        super().__init__(**kwargs)
        self._engine = mg.Engine(mg.EngineConfig(self.width, self.height))
        self._camera = camera or mg.Camera.framing_aabb(
            scene.world_aabb(), self.width / self.height
        )
        self._post_frame()

    @traitlets.observe("pointer_state")
    def _on_pointer_state(self, change):
        self._interact(json.loads(change["new"]))

    def _interact(self, state):
        """One coalesced pointer state in, one rendered frame out.

        Forwarding only: the event is built and applied in Rust, the profile
        switches per phase, and the frame the engine rendered is shipped.
        """
        event = self._event(state)
        if event is not None:
            self._controller.update(event, self._camera)
        if state.get("dragging"):
            self._engine.set_render_profile(mg.RenderProfile.inspection())
            self._post_frame(self.width // 2, self.height // 2)
        else:
            self._engine.set_render_profile(self._profile)
            self._post_frame()

    def _event(self, state):
        """The abstract ``InputEvent`` one browser state names, or None."""
        kind = state["kind"]
        if kind == "button":
            return mg.core.InputEvent.pointer_button(
                _BUTTONS[state["button"]], state["pressed"], state["x"], state["y"]
            )
        if kind == "move":
            return mg.core.InputEvent.pointer_move(state["x"], state["y"])
        if kind == "scroll":
            return mg.core.InputEvent.scroll(state["delta"])
        return None

    def _post_frame(self, width=None, height=None):
        """Render with the engine and publish the PNG it produced."""
        image = self._engine.render_image(
            self._scene, self._camera, width or self.width, height or self.height
        )
        self.frame_png = base64.b64encode(image.copy_png_bytes()).decode("ascii")