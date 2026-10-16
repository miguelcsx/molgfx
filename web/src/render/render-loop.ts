import { dimensions } from "./camera.js";
import type {
  WasmCamera,
  WasmPickReadback,
  WasmPickStream,
  WasmRenderer,
  WasmScene,
} from "../core/types.js";
export interface PickResult {
  performed: boolean;
  result?: string;
}
interface RenderCallbacks {
  onError(error: unknown): void;
  onSuccess(): void;
}
interface PendingHover {
  x: number;
  y: number;
  resolve(result: PickResult): void;
  reject(error: unknown): void;
}
interface HoverInFlight extends PendingHover {
  epoch: number;
}
interface StreamAnswer {
  stream: string;
  x: number;
  y: number;
  pick?: unknown;
  error?: string;
}
/** Wire value of `PickStream.Hover`; the enum itself is a runtime export. */
const HOVER_STREAM = 0 satisfies WasmPickStream;
/** One demand scheduler and one detached readback queue per canvas. */
export class RenderLoop {
  readonly #canvas: HTMLCanvasElement;
  #renderer: WasmRenderer;
  readonly #callbacks: RenderCallbacks;
  readonly #position = new Float32Array(3);
  readonly #target = new Float32Array(3);
  readonly #up = new Float32Array(3);
  #scene: WasmScene;
  #camera: WasmCamera | undefined;
  #cameraDirty = true;
  #sizeDirty = true;
  #pixelRatio = -1;
  #frame: number | undefined;
  #pickTail: Promise<void> = Promise.resolve();
  #hoverFrame: number | undefined;
  #hoverPending: PendingHover | undefined;
  #hoverInFlight: HoverInFlight | undefined;
  #streamFrame: number | undefined;
  #viewEpoch = 0;
  #hoverMemo:
    { x: number; y: number; epoch: number; result: PickResult } | undefined;
  #disposed = false;
  #continuous = false;
  #capture: readonly [number, number] | undefined;
  constructor(
    canvas: HTMLCanvasElement,
    renderer: WasmRenderer,
    scene: WasmScene,
    callbacks: RenderCallbacks,
  ) {
    this.#canvas = canvas;
    this.#renderer = renderer;
    this.#scene = scene;
    this.#callbacks = callbacks;
  }
  get disposed(): boolean {
    return this.#disposed;
  }
  get hasScene(): boolean {
    return this.#scene.isReady;
  }
  get camera(): WasmCamera {
    this.#assertLive();
    this.#ensureSize();
    this.#camera ??= this.#scene.camera(
      this.#canvas.width,
      this.#canvas.height,
    );
    return this.#camera;
  }
  setCamera(next: WasmCamera): void {
    if (this.#disposed) {
      next.free();
      return;
    }
    const previous = this.#camera;
    this.#camera = next;
    previous?.free();
    this.#cameraDirty = true;
    this.invalidatePicks();
    this.requestFrame();
  }
  /** Adopts a replacement renderer after a device loss; the caller frees the old one. */
  replaceRenderer(next: WasmRenderer): void {
    if (this.#disposed) return;
    this.#renderer = next;
    this.#sizeDirty = true;
    this.#cameraDirty = true;
    this.invalidatePicks();
    this.requestFrame();
  }
  setScene(next: WasmScene): void {
    this.#scene = next;
    this.invalidateCamera();
  }
  invalidatePicks(): void {
    this.#viewEpoch += 1;
    this.#hoverMemo = undefined;
  }
  invalidateCamera(): void {
    this.#camera?.free();
    this.#camera = undefined;
    this.#cameraDirty = true;
    this.invalidatePicks();
    this.requestFrame();
  }
  /** While true the loop redraws every frame (a playing transport); false restores on-demand drawing. */
  setContinuous(on: boolean): void {
    if (this.#disposed || this.#continuous === on) return;
    this.#continuous = on;
    if (on) this.requestFrame();
  }
  markSizeDirty = (): void => {
    if (this.#disposed) return;
    this.#sizeDirty = true;
    this.invalidatePicks();
    this.requestFrame();
  };
  /** Fixes the canvas to `width` by `height` and stops scheduling frames until `endCapture`. */
  beginCapture(width: number, height: number): void {
    this.#assertLive();
    if (this.#frame !== undefined) cancelAnimationFrame(this.#frame);
    this.#frame = undefined;
    this.#capture = [width, height];
    this.#sizeDirty = true;
  }
  /** Restores the viewport size and on-demand drawing. */
  endCapture(): void {
    this.#capture = undefined;
    if (this.#disposed) return;
    this.#sizeDirty = true;
    this.invalidatePicks();
    this.requestFrame();
  }
  /**
   * Renders the current scene synchronously until the renderer reports no
   * further accumulation, bounded by `passes`. It never schedules a frame.
   * Returns false when the bound was reached while still accumulating.
   */
  drawConverged(passes: number): boolean {
    this.#assertLive();
    this.#ensureSize();
    if (!this.hasScene) return true;
    const camera = this.camera;
    if (this.#cameraDirty) {
      this.#position.set(camera.position);
      this.#target.set(camera.target);
      this.#up.set(camera.up);
      this.#cameraDirty = false;
    }
    let converged = false;
    for (let pass = 0; pass < passes && !converged; pass += 1) {
      converged = !this.#renderer.renderCamera(
        this.#scene,
        this.#position,
        this.#target,
        this.#up,
      );
    }
    this.#callbacks.onSuccess();
    return converged;
  }
  requestFrame = (): void => {
    if (this.#disposed || this.#frame !== undefined || this.#capture) return;
    this.#frame = requestAnimationFrame(() => {
      this.#frame = undefined;
      try {
        this.drawNow();
      } catch (error) {
        this.#callbacks.onError(error);
      }
    });
  };
  /** Synchronous render failure is part of create/load readiness, not swallowed by RAF. */
  drawNow(): void {
    this.#assertLive();
    if (this.#frame !== undefined) {
      cancelAnimationFrame(this.#frame);
      this.#frame = undefined;
    }
    this.#ensureSize();
    if (!this.hasScene) return;
    const camera = this.camera;
    if (this.#cameraDirty) {
      this.#position.set(camera.position);
      this.#target.set(camera.target);
      this.#up.set(camera.up);
      this.#cameraDirty = false;
    }
    const again = this.#renderer.renderCamera(
      this.#scene,
      this.#position,
      this.#target,
      this.#up,
    );
    this.#callbacks.onSuccess();
    if (again || this.#continuous) this.requestFrame();
  }
  pick(x: number, y: number): Promise<PickResult> {
    if (this.#disposed || !this.hasScene)
      return Promise.resolve({ performed: false });
    const epoch = this.#viewEpoch;
    const result = this.#pickTail.then(async (): Promise<PickResult> => {
      if (this.#disposed || epoch !== this.#viewEpoch)
        return { performed: false };
      // Picking reuses settled buffers; a new temporal sample would move
      // subpixel coverage even though the camera and scene are unchanged.
      if (
        this.#frame !== undefined ||
        this.#cameraDirty ||
        this.#sizeDirty ||
        this.#pixelRatio !== (window.devicePixelRatio || 1)
      ) this.drawNow();
      if (epoch !== this.#viewEpoch) return { performed: false };
      const submittedEpoch = this.#viewEpoch;
      let readback: WasmPickReadback | undefined;
      try {
        readback = this.#renderer.beginPick(x, y);
        if (!readback) return { performed: true };
        const bytes = await readback.resolve();
        // Camera motion, DPR/resize, edits, scene replacement and disposal all
        // invalidate the *image* a readback belongs to, not just its atom table.
        if (this.#disposed || submittedEpoch !== this.#viewEpoch)
          return { performed: false };
        const picked = this.#renderer.finishPick(this.#scene, readback, bytes);
        return picked === undefined
          ? { performed: true }
          : { performed: true, result: picked };
      } finally {
        readback?.free();
      }
    });
    // A detached readback owns its GPU handles. It never gates the render RAF.
    this.#pickTail = result.then(
      () => {},
      () => {},
    );
    return result;
  }
  pickHover(x: number, y: number): Promise<PickResult> {
    if (this.#disposed) return Promise.resolve({ performed: false });
    const memo = this.#hoverMemo;
    if (memo && memo.x === x && memo.y === y && memo.epoch === this.#viewEpoch)
      return Promise.resolve(memo.result);
    const result = new Promise<PickResult>((resolve, reject) => {
      this.#hoverPending?.resolve({ performed: false });
      this.#hoverPending = { x, y, resolve, reject };
    });
    this.#scheduleHover();
    return result;
  }
  #scheduleHover(): void {
    if (
      this.#disposed ||
      this.#hoverFrame !== undefined ||
      this.#hoverInFlight ||
      !this.#hoverPending
    )
      return;
    this.#hoverFrame = requestAnimationFrame(() => {
      this.#hoverFrame = undefined;
      const pending = this.#hoverPending;
      this.#hoverPending = undefined;
      if (!pending || this.#disposed || !this.hasScene) {
        pending?.resolve({ performed: false });
        return;
      }
      try {
        // Picking reuses settled buffers; see `pick`.
        if (this.#needsDraw()) this.drawNow();
        this.#renderer.requestPick(HOVER_STREAM, pending.x, pending.y);
      } catch (error) {
        pending.reject(error);
        this.#scheduleHover();
        return;
      }
      this.#hoverInFlight = { ...pending, epoch: this.#viewEpoch };
      this.#pollStreams();
    });
  }
  #needsDraw(): boolean {
    return (
      this.#frame !== undefined ||
      this.#cameraDirty ||
      this.#sizeDirty ||
      this.#pixelRatio !== (window.devicePixelRatio || 1)
    );
  }
  /** One non-blocking poll per animation frame until the hover answers. */
  #pollStreams(): void {
    if (this.#streamFrame !== undefined) return;
    this.#streamFrame = requestAnimationFrame(() => {
      this.#streamFrame = undefined;
      const flight = this.#hoverInFlight;
      if (!flight || this.#disposed) return;
      try {
        if (this.#renderer.poll() > 0) {
          const answers = JSON.parse(
            this.#renderer.takePicks(this.#scene),
          ) as StreamAnswer[];
          let answer: StreamAnswer | undefined;
          for (const entry of answers)
            if (entry.stream === "hover") answer = entry;
          if (answer) {
            this.#finishHover(flight, answer);
            return;
          }
        }
      } catch (error) {
        this.#hoverInFlight = undefined;
        flight.reject(error);
        this.#scheduleHover();
        return;
      }
      this.#pollStreams();
    });
  }
  #finishHover(flight: HoverInFlight, answer: StreamAnswer): void {
    this.#hoverInFlight = undefined;
    if (answer.error !== undefined) {
      flight.reject(new Error(answer.error));
    } else {
      // Camera motion, resize, edits and scene replacement invalidate the
      // image a pick belongs to, exactly as for a detached readback.
      const result: PickResult =
        flight.epoch !== this.#viewEpoch
          ? { performed: false }
          : answer.pick === null || answer.pick === undefined
            ? { performed: true }
            : { performed: true, result: JSON.stringify(answer.pick) };
      if (result.performed)
        this.#hoverMemo = { x: flight.x, y: flight.y, epoch: flight.epoch, result };
      flight.resolve(result);
    }
    this.#scheduleHover();
  }
  settled(): Promise<void> {
    return this.#pickTail;
  }
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.invalidatePicks();
    if (this.#frame !== undefined) cancelAnimationFrame(this.#frame);
    if (this.#hoverFrame !== undefined) cancelAnimationFrame(this.#hoverFrame);
    if (this.#streamFrame !== undefined) cancelAnimationFrame(this.#streamFrame);
    this.#streamFrame = undefined;
    this.#hoverInFlight?.resolve({ performed: false });
    this.#hoverInFlight = undefined;
    this.#frame = undefined;
    this.#hoverFrame = undefined;
    this.#hoverPending?.resolve({ performed: false });
    this.#hoverPending = undefined;
    this.#camera?.free();
    this.#camera = undefined;
  }
  #ensureSize(): void {
    const ratio = window.devicePixelRatio || 1;
    if (!this.#sizeDirty && this.#pixelRatio === ratio) return;
    this.#sizeDirty = false;
    this.#pixelRatio = ratio;
    const [width, height] = this.#capture ?? dimensions(this.#canvas);
    if (this.#canvas.width !== width || this.#canvas.height !== height) {
      this.#canvas.width = width;
      this.#canvas.height = height;
      this.#renderer.resize(width, height);
      this.invalidatePicks();
    }
  }
  #assertLive(): void {
    if (this.#disposed) throw new Error("Viewer is disposed");
  }
}
