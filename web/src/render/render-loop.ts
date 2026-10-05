import { dimensions } from "./camera.js";
import type {
  WasmCamera,
  WasmPickReadback,
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
/** One demand scheduler and one detached readback queue per canvas. */
export class RenderLoop {
  readonly #canvas: HTMLCanvasElement;
  readonly #renderer: WasmRenderer;
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
  #hoverInFlight = false;
  #viewEpoch = 0;
  #hoverMemo:
    { x: number; y: number; epoch: number; result: PickResult } | undefined;
  #disposed = false;
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
  markSizeDirty = (): void => {
    if (this.#disposed) return;
    this.#sizeDirty = true;
    this.invalidatePicks();
    this.requestFrame();
  };
  requestFrame = (): void => {
    if (this.#disposed || this.#frame !== undefined) return;
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
    if (again) this.requestFrame();
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
      if (!pending || this.#disposed) {
        pending?.resolve({ performed: false });
        return;
      }
      this.#hoverInFlight = true;
      const epoch = this.#viewEpoch;
      void this.pick(pending.x, pending.y)
        .then((result) => {
          if (result.performed && epoch === this.#viewEpoch)
            this.#hoverMemo = { x: pending.x, y: pending.y, epoch, result };
          pending.resolve(result);
        }, pending.reject)
        .finally(() => {
          this.#hoverInFlight = false;
          this.#scheduleHover();
        });
    });
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
    const [width, height] = dimensions(this.#canvas);
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
