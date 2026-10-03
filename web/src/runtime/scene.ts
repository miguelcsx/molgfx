import type { SceneSnapshot } from "../core/contracts.js";
import type { RuntimeModule, WasmScene } from "../core/types.js";
import { sourceBytes } from "./runtime.js";

interface PatchOperation {
  op?: string;
  channel?: string;
  selection?: string | null;
}
export interface PatchDocument {
  operations?: PatchOperation[];
}

export interface PatchResult {
  applied: boolean;
  cameraChanged: boolean;
}

export function buildScene(
  snapshot: SceneSnapshot,
  runtime: RuntimeModule,
): WasmScene {
  const scene = new runtime.Scene(snapshot.spec);
  try {
    for (const structure of snapshot.structures) {
      scene.bindStructure(
        structure.id,
        sourceBytes(structure.payload),
        structure.name,
      );
    }
    // The engine requires a bound source for physical geometry. A valid empty
    // semantic snapshot stays unloaded until the first structure arrives.
    const declared = (
      JSON.parse(snapshot.spec) as { structures: Record<string, unknown> }
    ).structures;
    if (Object.keys(declared).length > 0) scene.resolve();
    return scene;
  } catch (error) {
    scene.free();
    throw error;
  }
}

/** Whether a committed patch moved the camera, so a cached view must be dropped. */
export function patchChangesCamera(patch: PatchDocument | undefined): boolean {
  return (
    patch?.operations?.some(
      ({ op }) => op === "set_camera" || op === "set_focus",
    ) ?? false
  );
}

export function applyScenePatch(
  scene: WasmScene,
  encoded: string,
  runtime: RuntimeModule,
): PatchResult {
  const patch = new runtime.ScenePatch(encoded);
  try {
    if (patch.baseRevision !== scene.revision)
      return { applied: false, cameraChanged: false };
    scene.apply(patch);
    return {
      applied: true,
      cameraChanged: patchChangesCamera(JSON.parse(encoded) as PatchDocument),
    };
  } finally {
    patch.free();
  }
}
