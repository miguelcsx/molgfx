import type { CommandBackend, InlineRuntimeSource, InteractionEvent, SceneSource, SequenceSink, ViewerSink } from "./core/contracts.js";
import { element, setAria } from "./core/dom.js";
import { errorMessage } from "./core/errors.js";
import { CleanupBag, listen } from "./core/lifecycle.js";
import { SceneGeneration } from "./core/interaction.js";
import type { Cleanup, WasmRenderer, WasmScene } from "./core/types.js";
import { mountInteractions } from "./interaction/interactions.js";
import { buildSequencePanelModel, displayedResidues, metadataFromScene, residueIntervals, residueNumberLabel, selectSequenceChain } from "./sequence.js";
import type { SequenceResidue } from "./sequence.js";
import { RenderLoop } from "./render/render-loop.js";
import { assertWebGPUAvailable, loadRuntime } from "./runtime/runtime.js";
import { mountScienceControls } from "./science-controls.js";
import { applyScenePatch, buildScene, sceneIsBehind } from "./runtime/scene.js";

/** Everything the reusable core needs from whatever is hosting it. */
export interface ViewerHost {
  el: HTMLElement;
  source: SceneSource;
  sink: ViewerSink;
  inlineRuntime?: InlineRuntimeSource;
  commandBackend?: CommandBackend;
  mountConsole?: (parent: HTMLElement) => Cleanup;
}


function createViewerDom(parent: HTMLElement) {
  const root = element("div", "molgfx-viewer");
  const canvas = element("canvas", "molgfx-canvas");
  const startup = element("div", "molgfx-startup", "Starting MolGFX…");
  const failure = element("pre", "molgfx-failure");
  canvas.tabIndex = 0;
  setAria(canvas, { label: "Molecular structure viewer", describedby: "molgfx-startup" });
  startup.setAttribute("role", "status");
  setAria(startup, { live: "polite" });
  failure.hidden = true;
  failure.setAttribute("role", "alert");
  root.append(canvas, startup, failure);
  parent.appendChild(root);
  return { root, canvas, startup, failure };
}

function mountSequencePanel(root: HTMLElement, scene: WasmScene, structure: bigint, transport: SequenceSink): Cleanup {
  let model = buildSequencePanelModel(metadataFromScene(scene, structure));
  const panel = element("section", "molgfx-sequence");
  setAria(panel, { label: "Structure sequence" });
  const heading = element("div", "molgfx-sequence-heading", "Sequence");
  const chooser = element("select", "molgfx-sequence-chain");
  setAria(chooser, { label: "Chain or entity" });
  const rows = element("div", "molgfx-sequence-rows");
  const actions = element("div", "molgfx-focus-actions");
  const ligand = element("button", "molgfx-focus-button", "Focus ligand");
  const selected = element("button", "molgfx-focus-button", "Focus selection");
  ligand.type = "button";
  selected.type = "button";
  actions.append(ligand, selected);
  panel.append(heading, chooser, rows, actions);
  root.appendChild(panel);

  let dragStart: number | undefined;
    let dragMoved = false;
  const emit = (residues: readonly SequenceResidue[]) => {
    const chain = model.chains.find((candidate) => candidate.key === model.selectedChain);
    if (!chain) return;
    const intervals = residueIntervals(residues);
    if (intervals.length > 0) transport.publishSequenceIntervals?.(intervals, chain.key, structure);
  };
  const render = () => {
    chooser.replaceChildren();
    for (const chain of model.chains) {
      const option = element("option", undefined, chain.label + (chain.entity === null ? "" : ` · entity ${chain.entity}`));
      option.value = chain.key;
      option.selected = chain.key === model.selectedChain;
      chooser.appendChild(option);
    }
    rows.replaceChildren();
    const chain = model.chains.find((candidate) => candidate.key === model.selectedChain);
    if (!chain) return;
    for (const residue of displayedResidues(chain)) {
      const row = element("button", "molgfx-sequence-residue");
      row.type = "button";
      row.disabled = residue.isGap;
      row.dataset.residueKey = residue.key;
      row.textContent = residue.oneLetter ?? residue.component ?? "·";
      row.title = `${residueNumberLabel(residue)}${residue.isGap ? " (canonical gap)" : ""}`;
      row.addEventListener("pointerdown", () => { dragStart = chain.residues.indexOf(residue); dragMoved = false; });
      row.addEventListener("pointerenter", () => {
        if (dragStart === undefined) return;
        const current = chain.residues.indexOf(residue);
                dragMoved = dragMoved || current !== dragStart;
        const from = Math.min(dragStart, current);
        const to = Math.max(dragStart, current);
        emit(chain.residues.slice(from, to + 1));
      });
      row.addEventListener("click", () => { if (dragMoved) { dragMoved = false; return; } emit([residue]); });
      rows.appendChild(row);
    }
  };
  chooser.addEventListener("change", () => { model = selectSequenceChain(model, chooser.value); render(); });
  const clearDrag = () => { dragStart = undefined; dragMoved = false; };
    window.addEventListener("pointerup", clearDrag, { passive: true });
  ligand.addEventListener("click", () => transport.publishFocusPreset?.("ligand"));
  selected.addEventListener("click", () => transport.publishFocusPreset?.("selection"));
  render();
  return () => { window.removeEventListener("pointerup", clearDrag); panel.remove(); };
}

export async function mountViewer(host: ViewerHost): Promise<Cleanup> {
  assertWebGPUAvailable();
  const { source, sink } = host;
  const dom = createViewerDom(host.el);
  const cleanup = new CleanupBag();
  let renderer: WasmRenderer | undefined;
  let scene: WasmScene | undefined;
  const generation = new SceneGeneration();
  let interactionSequence = 0;
  try {
    const runtime = await loadRuntime(host.inlineRuntime);
    scene = buildScene(source.snapshot(), runtime);
    renderer = await runtime.Renderer.create(dom.canvas);
    const loop = new RenderLoop(dom.canvas, renderer, scene, {
      onError(error) { sink.reportError(error); dom.failure.textContent = "MolGFX could not draw: " + errorMessage(error); dom.failure.hidden = false; },
      onSuccess() { dom.failure.hidden = true; },
    });
    // Disposal is ordered explicitly below: pending readbacks borrow wasm resources.
    // The finalizer below waits for them before stopping the scheduler.
    const resizeObserver = new ResizeObserver(loop.markSizeDirty);
    resizeObserver.observe(dom.canvas);
    cleanup.add(() => resizeObserver.disconnect());
    cleanup.add(listen(window, "resize", loop.markSizeDirty));
    const interactions = mountInteractions({
      canvas: dom.canvas, loop, publishCamera: (camera) => sink.publishCamera(camera),
      onPick: (x, y, modifiers) => {
        const requestGeneration = generation.value;
        void loop.pick(x, y).then(({ performed, result }) => {
          if (!performed || loop.disposed || !generation.isCurrent(requestGeneration)) return;
          const event: InteractionEvent = { kind: "pick", eventId: `${requestGeneration}:${++interactionSequence}`, ...(result === undefined ? {} : { result }), modifiers, sceneGeneration: requestGeneration };
          // One authoritative typed event; the legacy pick/selection traits are a
          // derived projection of the same result, never a second transport.
          sink.publishPick(event.result);
          sink.publishInteraction(event);
        }).catch((error) => sink.reportError(error));
      },
      onClearSelection: () => {
        sink.publishInteraction({ kind: "clear", eventId: `${generation.value}:${++interactionSequence}`, sceneGeneration: generation.value });
      },
      onHover: (result) => dom.canvas.classList.toggle("is-hovering-atom", result !== undefined),
    });
    cleanup.add(() => interactions.dispose());
    let sequencePanelCleanup: Cleanup | undefined;
        const mountCurrentSequencePanel = (current: WasmScene) => {
          sequencePanelCleanup?.();
          const firstStructure = source.snapshot().structures[0];
          sequencePanelCleanup = firstStructure ? mountSequencePanel(dom.root, current, firstStructure.id, sink) : undefined;
        };
        mountCurrentSequencePanel(scene);
        cleanup.add(() => { sequencePanelCleanup?.(); sequencePanelCleanup = undefined; });
    const replaceScene = () => {
      try {
        const next = buildScene(source.snapshot(), runtime); const previous = scene;
        interactions.cancelPendingPublish(); generation.advance(); scene = next; loop.setScene(next);
                mountCurrentSequencePanel(next);
        void loop.settled().then(() => previous?.free());
        if (sceneIsBehind(next, source)) source.requestResync();
      } catch (error) { sink.reportError(error); }
    };
    const patchScene = (patchJson: string) => {
      if (!scene) return;
      try {
        const result = applyScenePatch(scene, patchJson, runtime);
        if (!result.applied) { source.requestResync(); return; }
        generation.advance();
        loop.invalidatePicks();
        if (result.cameraChanged) { interactions.cancelPendingPublish(); loop.invalidateCamera(); }
        loop.requestFrame();
      } catch (error) { sink.reportError(error); }
    };
    cleanup.add(source.onReplace(replaceScene));
    cleanup.add(source.onPatch(patchScene));
    if (host.mountConsole) cleanup.add(host.mountConsole(dom.root));
    if (host.commandBackend) cleanup.add(mountScienceControls(host.commandBackend, dom.root));
    dom.startup.remove(); loop.requestFrame();
    if (sceneIsBehind(scene, source)) source.requestResync();
    let closed = false;
    return () => {
      if (closed) return;
      closed = true;
      generation.advance();
      cleanup.dispose();
      // completion must not observe a live loop and enqueue a final RAF after
      // the host has removed this viewer.
      loop.dispose();
      dom.root.remove();
      const closingScene = scene;
      const closingRenderer = renderer;
      scene = undefined;
      renderer = undefined;
      // A pick borrows wasm resources until its asynchronous readback settles.
      // The loop is already quiescent; only then is it safe to free wasm.
      void loop.settled().then(() => {
        closingScene?.free();
        closingRenderer?.free();
      });
    };
  } catch (error) {
    cleanup.dispose(); scene?.free(); renderer?.free(); dom.root.remove(); throw error;
  }
}
