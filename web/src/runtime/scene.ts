import type { SceneSnapshot, PlaybackUpdate } from "../core/contracts.js";
import type { RuntimeModule, WasmScene } from "../core/types.js";
import { sourceBytes } from "./runtime.js";
import type { BinaryState } from "../core/transport.js";

function words<T extends Float32Array | Uint32Array>(payload: BinaryState, Type: { new(buffer: ArrayBufferLike, offset: number, length: number): T }): T {
  const bytes = sourceBytes(payload);
  if (bytes.byteLength % 4 !== 0) throw new Error("scientific transport columns require complete 32-bit words");
  const aligned = bytes.byteOffset % 4 === 0 ? bytes : bytes.slice();
  return new Type(aligned.buffer, aligned.byteOffset, aligned.byteLength / 4);
}

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
    for (const binding of snapshot.bindings?.volumes ?? []) {
      scene.restoreVolume(binding.id, words(binding.values, Float32Array));
    }
    for (const frame of snapshot.bindings?.coordinateFrames ?? []) {
      scene.restoreCoordinateFrame(frame.id, frame.index, frame.seconds, words(frame.positions, Float32Array));
    }
    for (const binding of snapshot.bindings?.trajectories ?? []) {
      scene.restoreTrajectory(binding.id, binding.start, binding.end, binding.seconds);
    }
    for (const binding of snapshot.bindings?.playback ?? []) {
      scene.restorePlayback(binding.structure, binding.start, binding.end, binding.seconds);
    }
    for (const binding of snapshot.bindings?.hydrogenBonds ?? []) {
      scene.restoreHydrogenBonds(binding.id, words(binding.endpoints, Uint32Array), binding.candidateDistance, binding.pose);
    }
    for (const binding of snapshot.bindings?.molecularSurfaces ?? []) {
      scene.restoreMolecularSurface(binding.id, binding.metadata, words(binding.vertices, Float32Array), words(binding.normals, Float32Array), words(binding.faces, Uint32Array), words(binding.atoms, Uint32Array));
    }
    scene.resolve();
    return scene;
  } catch (error) {
    scene.free();
    throw error;
  }
}

export function applyPlayback(scene: WasmScene, update: PlaybackUpdate): void {
  const reference = (frame: PlaybackUpdate["start"]) => ({ identity: frame.id.toString(), index: frame.index.toString(), seconds: frame.seconds });
  const payload = (frame: PlaybackUpdate["start"]) => scene.hasCoordinateFrame(frame.id) ? new Float32Array(0) : words(frame.positions, Float32Array);
  scene.setTrajectoryPair(update.structure, JSON.stringify({ start: reference(update.start), end: reference(update.end), seconds: update.seconds }), payload(update.start), payload(update.end));
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
