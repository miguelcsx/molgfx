import type { ChunkRequest, TrajectoryChunk } from "../core/contracts.js";
import type { RuntimeModule, WasmScene } from "../core/types.js";
import { ChunkWindow } from "./trajectory-chunks.js";
import { applyPlayback } from "./scene.js";

/**
 * Routes trajectory samples through per-structure chunk windows. A structure
 * the kernel never sent a chunk for keeps the direct `setTrajectoryTime` path.
 */
export class TrajectoryStream {
  readonly #windows = new Map<bigint, ChunkWindow>();
  readonly #resident = new Map<bigint, string>();
  readonly #latest = new Map<bigint, number>();
  readonly #request: (request: ChunkRequest) => void;

  readonly #runtime: () => RuntimeModule | undefined;

  constructor(request: (request: ChunkRequest) => void, runtime: () => RuntimeModule | undefined) {
    this.#request = request;
    this.#runtime = runtime;
  }

  sample(scene: WasmScene, structure: bigint, seconds: number): void {
    const window = this.#windows.get(structure);
    if (!window) {
      scene.setTrajectoryTime(structure, seconds);
      return;
    }
    this.#latest.set(structure, seconds);
    this.#show(scene, structure, window, seconds);
  }

  receive(scene: WasmScene, chunk: TrajectoryChunk): void {
    let window = this.#windows.get(chunk.structure);
    if (!window) {
      window = new ChunkWindow();
      this.#windows.set(chunk.structure, window);
    }
    const seconds = this.#latest.get(chunk.structure) ?? chunk.frames[0]?.seconds ?? 0;
    window.accept(chunk.frames, seconds);
    if (this.#latest.has(chunk.structure)) this.#show(scene, chunk.structure, window, seconds);
  }

  /** A replaced scene holds no swapped pair; the window stays and re-applies on the next sample. */
  forgetResident(): void {
    this.#resident.clear();
  }

  #show(scene: WasmScene, structure: bigint, window: ChunkWindow, seconds: number): void {
    const pair = window.pairAt(seconds);
    if (pair) {
      const key = pair.start.id + ":" + pair.end.id;
      if (this.#resident.get(structure) !== key) {
        applyPlayback(scene, { structure, start: pair.start, end: pair.end, seconds }, this.#runtime());
        this.#resident.set(structure, key);
      } else {
        scene.setTrajectoryTime(structure, seconds);
      }
    }
    const request = window.next(seconds);
    if (request) this.#request({ structure, ...request });
  }
}
