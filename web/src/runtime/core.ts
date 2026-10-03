import { element } from "../core/dom.js";
import { CleanupBag, listen } from "../core/lifecycle.js";
import { ViewerEmitter } from "../core/events.js";
import { SceneGeneration } from "../core/interaction.js";
import { mountInteractions } from "../interaction/interactions.js";
import { RenderLoop } from "../render/render-loop.js";
import { cameraState, pickCoordinates } from "../render/camera.js";
import { assertWebGPUAvailable, loadRuntime, sourceBytes } from "./runtime.js";
import { applyScenePatch, buildScene, patchChangesCamera } from "./scene.js";
import type {
  ViewerHost,
  InteractionEvent,
  SceneSnapshot,
} from "../core/contracts.js";
import type {
  RuntimeModule,
  WasmRenderer,
  WasmScene,
  WasmSession,
} from "../core/types.js";
import type { ViewerInteractions } from "../interaction/interactions.js";
import type { PatchDocument } from "./scene.js";
import type {
  CommandResult,
  LoadOptions,
  Pick,
  SelectionModifiers,
  StructureInput,
  ViewerOptions,
} from "../types.js";
const EMPTY: SceneSnapshot = {
  spec: JSON.stringify({
    revision: 0,
    structures: {},
    representations: {},
    focus: null,
    selected: null,
    hovered: null,
    muted: null,
    hidden: null,
    extensions: {},
  }),
  structures: [],
};
const NO_MODIFIERS: SelectionModifiers = {
  shift: false,
  control: false,
  meta: false,
  alt: false,
};
interface Answer {
  ok: boolean;
  revision: number;
  messages?: string[];
  patch?: PatchDocument;
  rendered?: string;
  errors?: { message: string }[];
}
/** Shared browser host. Local Session edits and remote committed patches are mutually exclusive. */
export class ViewerCore {
  readonly events = new ViewerEmitter();
  readonly #cleanup = new CleanupBag();
  readonly #generation = new SceneGeneration();
  readonly #host: ViewerHost | undefined;
  readonly #root: HTMLDivElement;
  readonly #canvas: HTMLCanvasElement;
  readonly #failure: HTMLPreElement;
  #runtime: RuntimeModule | undefined;
  #scene: WasmScene | undefined;
  #session: WasmSession | undefined;
  #renderer: WasmRenderer | undefined;
  #loop: RenderLoop | undefined;
  #interactions: ViewerInteractions | undefined;
  #disposed = false;
  #closing: Promise<void> | undefined;
  #loadSequence = 0;
  #loading = false;
  #eventSequence = 0;
  constructor(el: HTMLElement, options: ViewerOptions, host?: ViewerHost) {
    this.#host = host;
    this.#root = element("div", "molgfx-viewer");
    this.#canvas = element("canvas", "molgfx-canvas");
    this.#failure = element("pre", "molgfx-failure");
    this.#failure.hidden = true;
    this.#failure.setAttribute("role", "alert");
    this.#canvas.tabIndex = 0;
    this.#canvas.setAttribute("aria-label", "Molecular structure viewer");
    this.#root.append(this.#canvas, this.#failure);
    el.append(this.#root);
    if (options.onError) this.events.on("error", options.onError);
    if (options.onPick) this.events.on("pick", options.onPick);
    if (options.onCamera) this.events.on("camera", options.onCamera);
    if (options.onSelectionChange)
      this.events.on("selectionchange", options.onSelectionChange);
    if (options.signal) {
      const signal = options.signal;
      const abort = () => {
        void this.dispose();
      };
      signal.addEventListener("abort", abort, { once: true });
      this.#cleanup.add(() => signal.removeEventListener("abort", abort));
      if (signal.aborted) abort();
    }
  }
  async initialize(options: ViewerOptions): Promise<void> {
    try {
      this.#assertLive();
      assertWebGPUAvailable();
      this.#runtime = await loadRuntime(this.#host?.inlineRuntime);
      this.#assertLive();
      this.#scene = buildScene(
        this.#host?.source.snapshot() ?? EMPTY,
        this.#runtime,
      );
      const renderer = await this.#runtime.Renderer.create(this.#canvas);
      if (this.#disposed) {
        renderer.free();
        this.#assertLive();
      }
      this.#renderer = renderer;
      this.#loop = new RenderLoop(this.#canvas, renderer, this.#scene, {
        onError: (error) => this.reportError(error),
        onSuccess: () => {
          this.#failure.hidden = true;
        },
      });
      this.#interactions = mountInteractions({
        canvas: this.#canvas,
        loop: this.#loop,
        runtime: this.#runtime,
        onError: (error) => this.reportError(error),
        publishCamera: (camera) => {
          if (this.#disposed) return;
          this.events.emit("camera", camera);
          this.#host?.sink.publishCamera(camera);
        },
        onPick: (x, y, modifiers) => {
          void this.#pickPixel(x, y, modifiers).catch((error) =>
            this.reportError(error),
          );
        },
        onClearSelection: () => {
          if (this.#host)
            this.#host.sink.publishInteraction({
              kind: "clear",
              eventId: this.#nextEventId(),
              sceneGeneration: this.#generation.value,
            });
          else this.select();
        },
      });
      this.#cleanup.add(() => this.#interactions?.dispose());
      const observer = new ResizeObserver(this.#loop.markSizeDirty);
      observer.observe(this.#canvas);
      this.#cleanup.add(() => observer.disconnect());
      this.#cleanup.add(listen(window, "resize", this.#loop.markSizeDirty));
      // A stationary page must still notice a DPR change when moved to another display.
      let query: MediaQueryList | undefined;
      const watchDpr = () => {
        query?.removeEventListener("change", watchDpr);
        query = matchMedia("(resolution: " + window.devicePixelRatio + "dppx)");
        query.addEventListener("change", watchDpr, { once: true });
        this.#loop?.markSizeDirty();
      };
      watchDpr();
      this.#cleanup.add(() => query?.removeEventListener("change", watchDpr));
      if (this.#host) {
        this.#cleanup.add(
          this.#host.source.onReplace(() => this.#replaceRemote()),
        );
        this.#cleanup.add(
          this.#host.source.onPatch((encoded) => this.#applyRemote(encoded)),
        );
        if (this.#scene.revision !== this.#host.source.revision())
          this.#replaceRemote();
      }
      this.#loop.drawNow();
      if (options.structure !== undefined)
        await this.load(
          options.structure,
          options.name === undefined ? {} : { name: options.name },
        );
    } catch (error) {
      this.reportError(error);
      await this.dispose();
      throw error;
    }
  }
  async load(input: StructureInput, options: LoadOptions = {}): Promise<void> {
    this.#assertLive();
    if (this.#host)
      throw new Error("A notebook viewer is owned by its kernel Session");
    const sequence = ++this.#loadSequence;
    this.#loading = true;
    this.#loop?.invalidatePicks();
    try {
      const file = typeof File !== "undefined" && input instanceof File;
      const bytes = file
        ? new Uint8Array(await input.arrayBuffer())
        : sourceBytes(input);
      this.#assertLive();
      if (sequence !== this.#loadSequence)
        throw new DOMException("Load superseded", "AbortError");
      const next = this.#runtime!.Scene.fromStructureBytes(
        bytes,
        options.name ?? (file ? input.name : "structure"),
      );
      this.#replace(next);
      this.events.emit("selectionchange", { selection: "" });
    } catch (error) {
      if (sequence === this.#loadSequence) this.reportError(error);
      throw error;
    } finally {
      if (sequence === this.#loadSequence) this.#loading = false;
    }
  }
  execute(text: string): CommandResult {
    this.#assertLive();
    if (this.#host)
      throw new Error("A notebook viewer is owned by its kernel Session");
    if (this.#loading)
      throw new Error("Wait for load() before editing the scene");
    try {
      const scene = this.#scene!;
      this.#session ??= new this.#runtime!.Session(scene);
      const answer = JSON.parse(this.#session.execute(scene, text)) as Answer;
      if (!answer.ok)
        throw new Error(
          answer.rendered ??
            answer.errors?.map((entry) => entry.message).join("\n") ??
            "Session rejected command",
        );
      this.#sceneEdited(patchChangesCamera(answer.patch));
      this.#selectionChanged(answer.patch);
      return { revision: answer.revision, messages: answer.messages ?? [] };
    } catch (error) {
      this.reportError(error);
      throw error;
    }
  }
  select(query?: string): CommandResult {
    return this.execute(
      query === undefined || query === "" ? "selection" : "selection " + query,
    );
  }
  /** CSS-local coordinates; stale readbacks resolve to undefined without publication. */
  async pick(x: number, y: number): Promise<Pick | undefined> {
    this.#assertLive();
    const bounds = this.#canvas.getBoundingClientRect();
    const point = pickCoordinates(
      this.#canvas,
      bounds.left + x,
      bounds.top + y,
    );
    return point ? this.#pickPixel(...point, NO_MODIFIERS) : undefined;
  }
  async #pickPixel(
    x: number,
    y: number,
    modifiers: SelectionModifiers,
  ): Promise<Pick | undefined> {
    const generation = this.#generation.value;
    const picked = await this.#loop!.pick(x, y);
    if (
      this.#disposed ||
      !picked.performed ||
      !this.#generation.isCurrent(generation)
    )
      return undefined;
    const value =
      picked.result === undefined
        ? undefined
        : (JSON.parse(picked.result) as Pick);
    const event: InteractionEvent = {
      kind: "pick",
      eventId: this.#nextEventId(),
      modifiers,
      sceneGeneration: generation,
      ...(picked.result === undefined ? {} : { result: picked.result }),
    };
    this.events.emit("pick", { result: value, modifiers });
    if (!this.#disposed) this.#host?.sink.publishInteraction(event);
    return value;
  }
  #nextEventId(): string {
    return this.#generation.value + ":" + ++this.#eventSequence;
  }
  #replace(next: WasmScene): void {
    const previous = this.#scene;
    const loop = this.#loop!;
    try {
      this.#interactions?.cancelPendingGesture();
      this.#interactions?.cancelPendingPublish();
      loop.setScene(next);
      loop.drawNow();
    } catch (error) {
      if (previous) loop.setScene(previous);
      next.free();
      throw error;
    }
    this.#generation.advance();
    this.#scene = next;
    this.#session?.free();
    this.#session = undefined;
    void loop.settled().then(() => previous?.free());
  }
  #sceneEdited(cameraChanged: boolean): void {
    this.#generation.advance();
    this.#loop!.invalidatePicks();
    if (cameraChanged) {
      this.#interactions?.cancelPendingPublish();
      this.#loop!.invalidateCamera();
      const camera = cameraState(this.#loop!.camera);
      this.events.emit("camera", camera);
      if (!this.#disposed) this.#host?.sink.publishCamera(camera);
    }
    this.#loop!.requestFrame();
  }
  #replaceRemote(): void {
    if (this.#disposed) return;
    try {
      this.#replace(buildScene(this.#host!.source.snapshot(), this.#runtime!));
    } catch (error) {
      this.reportError(error);
      this.#host!.source.requestResync();
      return;
    }
    if (this.#scene!.revision !== this.#host!.source.revision())
      this.#host!.source.requestResync();
  }
  #applyRemote(encoded: string): void {
    if (this.#disposed) return;
    try {
      const result = applyScenePatch(this.#scene!, encoded, this.#runtime!);
      if (!result.applied) {
        this.#host!.source.requestResync();
        return;
      }
      this.#sceneEdited(result.cameraChanged);
      this.#selectionChanged(JSON.parse(encoded) as PatchDocument);
    } catch (error) {
      this.reportError(error);
      this.#host!.source.requestResync();
    }
  }
  #selectionChanged(patch: PatchDocument | undefined): void {
    const operations = patch?.operations ?? [];
    for (let index = operations.length - 1; index >= 0; index--) {
      const operation = operations[index];
      if (
        operation?.op === "set_interaction" &&
        operation.channel === "selected"
      ) {
        this.events.emit("selectionchange", {
          selection: operation.selection ?? "",
        });
        return;
      }
    }
  }
  reportError(error: unknown): void {
    if (this.#disposed) return;
    const value = error instanceof Error ? error : new Error(String(error));
    this.#failure.textContent = value.message;
    this.#failure.hidden = false;
    this.#host?.sink.reportError(value);
    this.events.emit("error", value);
  }
  dispose(): Promise<void> {
    if (this.#closing) return this.#closing;
    this.#disposed = true;
    ++this.#loadSequence;
    this.#generation.advance();
    this.#cleanup.dispose();
    this.#loop?.dispose();
    this.events.dispose();
    this.#root.remove();
    const scene = this.#scene,
      renderer = this.#renderer,
      session = this.#session;
    this.#scene = undefined;
    this.#renderer = undefined;
    this.#session = undefined;
    this.#closing = (this.#loop?.settled() ?? Promise.resolve()).then(() => {
      session?.free();
      scene?.free();
      renderer?.free();
    });
    return this.#closing;
  }
  #assertLive(): void {
    if (this.#disposed)
      throw new DOMException("Viewer is disposed", "AbortError");
  }
}
