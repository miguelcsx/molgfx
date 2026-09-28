import { dimensions, sceneCamera } from "./camera.js";
import type { Camera, WasmRenderer, WasmScene } from "../core/types.js";

export interface PickResult { performed: boolean; result?: string; }
interface RenderCallbacks { onError: (error: unknown) => void; onSuccess: () => void; }

export class RenderLoop {
  readonly #canvas: HTMLCanvasElement;
  readonly #renderer: WasmRenderer;
  readonly #callbacks: RenderCallbacks;
  readonly #position = new Float32Array(3);
  readonly #target = new Float32Array(3);
  readonly #up = new Float32Array(3);
  #scene: WasmScene;
  #camera: Camera | undefined;
  #sizeDirty = true;
  #pixelRatio = -1;
  #frame: number | null = null;
  #picking = false;
  #redrawAfterPick = false;
  #pickInFlight: Promise<PickResult> | undefined;
  #pickTail: Promise<void> = Promise.resolve();
  #sceneEpoch = 0;
  #disposed = false;
  #failed = false;

  constructor(canvas: HTMLCanvasElement, renderer: WasmRenderer, scene: WasmScene, callbacks: RenderCallbacks) {
    this.#canvas = canvas; this.#renderer = renderer; this.#scene = scene; this.#callbacks = callbacks;
  }
  get camera(): Camera { const [width, height] = this.#ensureSize(); this.#camera ??= sceneCamera(this.#scene, width, height); return this.#camera; }
  get disposed(): boolean { return this.#disposed; }
  setScene(scene: WasmScene): void { this.#scene = scene; this.#sceneEpoch += 1; this.invalidateCamera(); this.requestFrame(); }
  invalidatePicks(): void { this.#sceneEpoch += 1; }
  invalidateCamera(): void { this.#camera = undefined; }
  markSizeDirty = (): void => { this.#sizeDirty = true; this.requestFrame(); };
  requestFrame = (): void => {
    if (this.#disposed) return;
    if (this.#picking) { this.#redrawAfterPick = true; return; }
    if (this.#frame !== null) return;
    this.#frame = requestAnimationFrame(() => { this.#frame = null; this.#draw(); });
  };
  async pick(x: number, y: number): Promise<PickResult> {
    if (this.#disposed) return { performed: false };
    let resolveResult!: (result: PickResult) => void;
    let rejectResult!: (error: unknown) => void;
    const result = new Promise<PickResult>((resolve, reject) => { resolveResult = resolve; rejectResult = reject; });
    const run = this.#pickTail.then(async () => {
      if (this.#disposed) { resolveResult({ performed: false }); return; }
      this.#picking = true;
      const inFlight = this.#runPick(x, y, this.#sceneEpoch);
      this.#pickInFlight = inFlight;
      try { resolveResult(await inFlight); }
      catch (error) { rejectResult(error); }
      finally { if (this.#pickInFlight === inFlight) this.#pickInFlight = undefined; }
    });
    this.#pickTail = run.catch(() => undefined);
    return result;
  }
  async #runPick(x: number, y: number, epoch: number): Promise<PickResult> {
    try {
      const result = await this.#renderer.pick(x, y);
      if (epoch !== this.#sceneEpoch || this.#disposed) return { performed: false };
      return result === undefined ? { performed: true } : { performed: true, result };
    } finally {
      this.#picking = false;
      if (this.#redrawAfterPick && !this.#disposed) { this.#redrawAfterPick = false; this.requestFrame(); }
    }
  }
  async settled(): Promise<void> { try { await this.#pickTail; } catch { /* readback errors are reported to the request */ } }
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true; this.#sceneEpoch += 1; this.#redrawAfterPick = false;
    if (this.#frame !== null) cancelAnimationFrame(this.#frame);
    this.#frame = null;
  }
  #ensureSize(): [number, number] {
    const pixelRatio = window.devicePixelRatio || 1;
    if (!this.#sizeDirty && this.#pixelRatio === pixelRatio) return [this.#canvas.width, this.#canvas.height];
    this.#sizeDirty = false; this.#pixelRatio = pixelRatio;
    const [width, height] = dimensions(this.#canvas);
    if (this.#canvas.width !== width || this.#canvas.height !== height) { this.#canvas.width = width; this.#canvas.height = height; this.#renderer.resize(width, height); }
    return [width, height];
  }
  #draw(): void {
    if (this.#disposed) return;
    if (this.#picking) { this.#redrawAfterPick = true; return; }
    try {
      const camera = this.camera; this.#position.set(camera.position); this.#target.set(camera.target); this.#up.set(camera.up);
      const needsAnotherFrame = this.#renderer.renderCamera(this.#scene, this.#position, this.#target, this.#up);
      if (this.#failed) { this.#failed = false; this.#callbacks.onSuccess(); }
      if (needsAnotherFrame) this.requestFrame();
    } catch (error) { this.#failed = true; this.#callbacks.onError(error); }
  }
}
