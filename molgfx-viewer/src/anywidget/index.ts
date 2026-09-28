// The anywidget ESM entry point: https://anywidget.dev's `{ render }` contract.
import { mountConsole } from "../console/console.js";
import { mountScienceControls } from "../science-controls.js";
import { PIXEL_BUDGET } from "../core/config.js";
import { element } from "../core/dom.js";
import { errorMessage } from "../core/errors.js";
import { dimensions } from "../render/camera.js";
import { loadRuntime as loadRuntimeCore, sourceBytes } from "../runtime/runtime.js";
import { mountViewer } from "../viewer.js";
import { AnywidgetAdapter } from "./adapter.js";
import { AnywidgetCommandBackend } from "./command-backend.js";
import type { RenderContext, WidgetModel } from "./model.js";
import { metadataFromScene } from "../sequence.js";
import type { ResidueMetadata } from "../sequence.js";

export function loadRuntime(model: WidgetModel) { return loadRuntimeCore(new AnywidgetAdapter(model)); }
export { dimensions, PIXEL_BUDGET, sourceBytes };
export type { ResidueMetadata } from "../sequence.js";
export { buildSequencePanelModel, metadataFromScene, residueNumberLabel, selectSequenceChain, wrappedOneLetter } from "../sequence.js";
export function residueMetadata(scene: { residueMetadataJSON(structure: number): string }, structure = 1): ResidueMetadata[] { return metadataFromScene(scene, structure); }

async function render({ model, el }: RenderContext) {
  const adapter = new AnywidgetAdapter(model);
  const workbenchConsole = model.get("workbench")
    ? {
        mountConsole: (parent: HTMLElement) => {
          const backend = new AnywidgetCommandBackend(model);
          const disposeConsole = mountConsole(backend, parent);
          const disposeControls = mountScienceControls(backend, parent);
          return () => {
            disposeControls();
            disposeConsole();
          };
        },
      }
    : {};
  try { return await mountViewer({ el, source: adapter, sink: adapter, inlineRuntime: adapter, ...workbenchConsole }); }
  catch (error) { adapter.reportError(error); const message = element("pre", "molgfx-failure", "MolGFX viewer could not start: " + errorMessage(error)); message.setAttribute("role", "alert"); el.appendChild(message); throw error; }
}
export default { render };