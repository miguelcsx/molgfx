// The anywidget ESM entry point: https://anywidget.dev's `{ render }` contract.
import { mountConsole } from "../console/console.js";
import { PIXEL_BUDGET } from "../core/config.js";
import { element } from "../core/dom.js";
import { errorMessage } from "../core/errors.js";
import { dimensions } from "../render/camera.js";
import { loadRuntime as loadRuntimeCore, sourceBytes } from "../runtime/runtime.js";
import { mountViewer } from "../viewer.js";
import { AnywidgetAdapter } from "../anywidget/adapter.js";
import { AnywidgetCommandBackend } from "../anywidget/command-backend.js";
import type { RenderContext, WidgetModel } from "../anywidget/model.js";

export function loadRuntime(model: WidgetModel) { return loadRuntimeCore(new AnywidgetAdapter(model)); }
export { dimensions, PIXEL_BUDGET, sourceBytes };

async function render({ model, el }: RenderContext) {
  const adapter = new AnywidgetAdapter(model);
  const workbenchConsole = model.get("workbench")
    ? {
        mountConsole: (parent: HTMLElement) => {
          return mountConsole(new AnywidgetCommandBackend(model), parent);
        },
      }
    : {};
  try { return await mountViewer({ el, source: adapter, sink: adapter, inlineRuntime: adapter, ...workbenchConsole }); }
  catch (error) { adapter.reportError(error); const message = element("pre", "molgfx-failure", "MolGFX viewer could not start: " + errorMessage(error)); message.setAttribute("role", "alert"); el.appendChild(message); throw error; }
}
export default { render };