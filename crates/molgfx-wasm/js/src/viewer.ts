import type { InlineRuntimeSource, InteractionEvent, SceneSource, ViewerSink } from "./core/contracts.js";
import { element, setAria } from "./core/dom.js";
import { errorMessage } from "./core/errors.js";
import { CleanupBag, listen } from "./core/lifecycle.js";
import { SceneGeneration } from "./core/interaction.js";
import type { Cleanup, RuntimeModule, WasmRenderer, WasmScene } from "./core/types.js";
import { mountInteractions } from "./interaction/interactions.js";
import { RenderLoop } from "./render/render-loop.js";
import { assertWebGPUAvailable, loadRuntime } from "./runtime/runtime.js";
import { applyScenePatch, buildScene, sceneIsBehind } from "./runtime/scene.js";

/**
 * The mounted view as a console sees it: the one live wasm scene, and the
 * single path an in-page edit takes to reach the screen. A host whose
 * commands run elsewhere (a kernel) ignores it; a page-local host edits this
 * scene in place instead of keeping a second copy.
 */
export interface ViewerContext {
  readonly runtime: RuntimeModule;
  scene(): WasmScene | undefined;
  /** Called after each scene replacement, with the new scene. */
  onSceneReplaced(callback: (scene: WasmScene) => void): Cleanup;
  /** The scene was edited in place; redraw it and drop stale picks. */
  sceneEdited(cameraChanged: boolean): void;
}

/** Everything the reusable core needs from whatever is hosting it. */
export interface ViewerHost {
  el: HTMLElement;
  source: SceneSource;
  sink: ViewerSink;
  inlineRuntime?: InlineRuntimeSource;
  mountConsole?: (parent: HTMLElement, view: ViewerContext) => Cleanup;
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

type PickAtom = {
  pick?: unknown;
  chain?: unknown;
  residue_number?: unknown;
  atom_name?: unknown;
};

function safeQueryToken(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z0-9_.+\-]+$/.test(value);
}

function atomSelection(result: string | undefined): string | undefined {
  if (result === undefined) return undefined;
  let value: unknown;
  try { value = JSON.parse(result) as unknown; } catch { return undefined; }
  if (!value || typeof value !== "object") return undefined;
  const atom = value as PickAtom;
  if (atom.pick !== "atom" || !Number.isInteger(atom.residue_number)) return undefined;
  const terms = [`resid ${String(atom.residue_number)}`];
  if (safeQueryToken(atom.chain)) terms.unshift(`chain ${atom.chain}`);
  return terms.join(" and ");
}

export async function mountViewer(host: ViewerHost): Promise<Cleanup> {
  assertWebGPUAvailable();
  const { source, sink } = host;
  const dom = createViewerDom(host.el);
  const cleanup = new CleanupBag();
  let renderer: WasmRenderer | undefined;
  let scene: WasmScene | undefined;
  const generation = new SceneGeneration();
  const localFile = "file" in source.snapshot();
  let localSelection: string | undefined;
  let localHover: string | undefined;
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
          const query = atomSelection(event.result);
          if (localFile && query !== undefined) {
            localSelection = event.modifiers.alt
              ? localSelection === undefined
                ? undefined
                : `(${localSelection}) and not (${query})`
              : event.modifiers.shift && localSelection !== undefined
                ? `(${localSelection}) or (${query})`
                : query;
            applyLocalInteraction("selected", localSelection);
          } else if (localFile && event.result === undefined) {
            localSelection = undefined;
            applyLocalInteraction("selected", undefined);
          }
        }).catch((error) => sink.reportError(error));
      },
      onClearSelection: () => {
        sink.publishInteraction({ kind: "clear", eventId: `${generation.value}:${++interactionSequence}`, sceneGeneration: generation.value });
        if (localFile) {
          localSelection = undefined;
          applyLocalInteraction("selected", undefined);
        }
      },
      onHover: (result) => {
        dom.canvas.classList.toggle("is-hovering-atom", result !== undefined);
        if (!localFile) return;
        const query = atomSelection(result);
        if (query === localHover) return;
        localHover = query;
        applyLocalInteraction("hovered", query);
      },
    });
    cleanup.add(() => interactions.dispose());
    const replaced = new Set<(scene: WasmScene) => void>();
    const replaceScene = () => {
      try {
        const next = buildScene(source.snapshot(), runtime); const previous = scene;
        interactions.cancelPendingPublish(); generation.advance(); scene = next; loop.setScene(next);
        localSelection = undefined; localHover = undefined;
        for (const callback of replaced) callback(next);
        void loop.settled().then(() => previous?.free());
        if (sceneIsBehind(next, source)) source.requestResync();
      } catch (error) { sink.reportError(error); }
    };
    const sceneEdited = (cameraChanged: boolean) => {
      generation.advance();
      loop.invalidatePicks();
      if (cameraChanged) { interactions.cancelPendingPublish(); loop.invalidateCamera(); }
      loop.requestFrame();
    };
    const applyLocalInteraction = (channel: "selected" | "hovered", selection: string | undefined) => {
      if (!localFile || !scene) return;
      const patch = JSON.stringify({
        base_revision: Number(scene.revision),
        operations: [{ op: "set_interaction", channel, selection: selection ?? null }],
      });
      try {
        const result = applyScenePatch(scene, patch, runtime);
        if (result.applied) sceneEdited(false);
      } catch (error) { sink.reportError(error); }
    };

    const patchScene = (patchJson: string) => {
      if (!scene) return;
      try {
        const result = applyScenePatch(scene, patchJson, runtime);
        if (!result.applied) { source.requestResync(); return; }
        sceneEdited(result.cameraChanged);
      } catch (error) { sink.reportError(error); }
    };
    const view: ViewerContext = {
      runtime,
      scene: () => scene,
      onSceneReplaced(callback) { replaced.add(callback); return () => { replaced.delete(callback); }; },
      sceneEdited,
    };
    cleanup.add(source.onReplace(replaceScene));
    cleanup.add(source.onPatch(patchScene));
    if (host.mountConsole) cleanup.add(host.mountConsole(dom.root, view));
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
