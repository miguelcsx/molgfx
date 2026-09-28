import type { SceneSnapshot, SceneSource } from "../core/contracts.js";
import type { RuntimeModule, WasmScene } from "../core/types.js";
import { sourceBytes } from "./runtime.js";

interface PatchOperation { op?: string; }
interface PatchDocument { operations?: PatchOperation[]; }

export interface PatchResult { applied: boolean; cameraChanged: boolean; }

export function buildScene(snapshot: SceneSnapshot, runtime: RuntimeModule): WasmScene {
  const scene = new runtime.Scene(snapshot.spec);
  try {
    for (const structure of snapshot.structures) {
      scene.bindStructure(structure.id, sourceBytes(structure.payload), structure.name);
    }
    scene.resolve();
    return scene;
  } catch (error) {
    scene.free();
    throw error;
  }
}

export function sceneIsBehind(scene: WasmScene, source: SceneSource): boolean {
  return scene.revision !== source.revision();
}

export function applyScenePatch(scene: WasmScene, encoded: string, runtime: RuntimeModule): PatchResult {
  const patch = new runtime.ScenePatch(encoded);
  try {
    if (patch.baseRevision !== scene.revision) return { applied: false, cameraChanged: false };
    scene.apply(patch);
    const document = JSON.parse(encoded) as PatchDocument;
    return {
      applied: true,
      cameraChanged: document.operations?.some(({ op }) => op === "set_camera") ?? false,
    };
  } finally {
    patch.free();
  }
}

/** Latest-only trajectory time delivery; frame uploads remain explicit. */
export class TrajectoryTimeCoalescer {
  #pending: { structure: bigint; seconds: number; topologyRevision: bigint } | undefined;
  #scheduled = false;
  #disposed = false;

  constructor(private readonly deliver: (structure: bigint, seconds: number, topologyRevision: bigint) => void) {}

  request(structure: bigint, seconds: number, topologyRevision: bigint): void {
    if (this.#disposed || !Number.isFinite(seconds)) return;
    this.#pending = { structure, seconds, topologyRevision };
    if (this.#scheduled) return;
    this.#scheduled = true;
    queueMicrotask(() => {
      this.#scheduled = false;
      const pending = this.#pending;
      this.#pending = undefined;
      if (!this.#disposed && pending) this.deliver(pending.structure, pending.seconds, pending.topologyRevision);
    });
  }

  cancel(): void { this.#pending = undefined; }
  dispose(): void { this.#disposed = true; this.#pending = undefined; }
}
