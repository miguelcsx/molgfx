import type {
  InlineRuntimeSource,
  InteractionEvent,
  SceneSnapshot,
  SceneSource,
  ViewerSink,
  PlaybackUpdate,
} from "../core/contracts.js";
import type { Camera, Cleanup } from "../core/transport.js";
import { observeModel } from "./model.js";
import type { WidgetModel } from "./model.js";

/** Adapts one anywidget `WidgetModel` to every port the viewer core needs. */
export class AnywidgetAdapter
  implements SceneSource, InlineRuntimeSource, ViewerSink
{
  readonly #model: WidgetModel;

  constructor(model: WidgetModel) {
    this.#model = model;
  }

  snapshot(): SceneSnapshot {
    const model = this.#model;
    const ids = model.get("structure_ids");
    const names = model.get("structure_names");
    const payloads = model.get("structure_payloads");
    const sources = model.get("structure_sources") ?? ids;
    if (
      ids.length !== names.length ||
      ids.length !== payloads.length ||
      ids.length !== sources.length
    ) {
      throw new Error("structure transport columns have different lengths");
    }
    const owned = new Map(
      ids.map((id, index) => [BigInt(id), payloads[index]]),
    );
    return {
      spec: model.get("scene_spec"),
      bindings: {
        coordinateFrames: (model.get("coordinate_frames") ?? []).map(([id, index, seconds, positions]) => ({ id: BigInt(id), index: BigInt(index), seconds, positions })),
        trajectories: (model.get("trajectory_bindings") ?? []).map(([id, start, end, seconds]) => ({ id: BigInt(id), start: BigInt(start), end: BigInt(end), seconds })),
        playback: (model.get("playback_bindings") ?? []).map(([id, start, end, seconds]) => ({ structure: BigInt(id), start: BigInt(start), end: BigInt(end), seconds })),
        volumes: (model.get("volume_bindings") ?? []).map(([id, values]) => ({ id: BigInt(id), values })),
        hydrogenBonds: (model.get("hydrogen_bond_bindings") ?? []).map(([id, endpoints, candidateDistance, pose]) => ({ id: BigInt(id), endpoints, pose, ...(candidateDistance === null ? {} : { candidateDistance }) })),
        molecularSurfaces: (model.get("molecular_surface_bindings") ?? []).map(([id, metadata, vertices, normals, faces, atoms]) => ({ id: BigInt(id), metadata, vertices, normals, faces, atoms })),
      },
      structures: ids.map((id, index) => {
        const source = sources[index];
        const payload =
          source === undefined ? undefined : owned.get(BigInt(source));
        if (id === undefined || payload === undefined) {
          throw new Error(
            `missing structure transport value at index ${index}`,
          );
        }
        return { id: BigInt(id), name: names[index] ?? "", payload };
      }),
    };
  }

  revision(): bigint {
    return BigInt(this.#model.get("revision"));
  }

  onReplace(callback: () => void): Cleanup {
    return observeModel(this.#model, "change:scene_spec", callback);
  }

  onPatch(callback: (patchJson: string) => void): Cleanup {
    return observeModel(this.#model, "change:patch_sequence", () =>
      callback(this.#model.get("scene_patch")),
    );
  }

  onTrajectorySample(callback: (structure: bigint, seconds: number) => void): Cleanup {
    return observeModel(this.#model, "change:trajectory_sample", () => {
      const sample = this.#model.get("trajectory_sample");
      if (sample?.[0]) callback(BigInt(sample[0]), sample[1]);
    });
  }

  onTrajectoryPair(callback: (update: PlaybackUpdate) => void): Cleanup {
    return observeModel(this.#model, "change:playback_pair", () => {
      const interval = this.#model.get("playback_pair");
      if (!interval?.[0]) return;
      const frame = (identity: string) => {
        const row = this.#model.get("coordinate_frames")?.find(([id]) => id === identity);
        if (!row) throw new Error("runtime playback requires its coordinate frame column");
        return { id: BigInt(row[0]), index: BigInt(row[1]), seconds: row[2], positions: row[3] };
      };
      callback({ structure: BigInt(interval[0]), start: frame(interval[1]), end: frame(interval[2]), seconds: interval[3] });
    });
  }

  requestResync(): void {
    this.#model.set("sync_request", (this.#model.get("sync_request") || 0) + 1);
    this.#model.save_changes();
  }

  glue(): string | undefined {
    return this.#model.get("_runtime_js");
  }
  key(): string | undefined {
    return this.#model.get("_runtime_key");
  }
  wasmGzip() {
    return this.#model.get("_runtime_wasm");
  }

  reportError(error: unknown): void {
    this.#model.set(
      "error",
      error instanceof Error ? error.message : String(error),
    );
    this.#model.save_changes();
  }

  publishCamera(camera: Camera): void {
    this.#model.set("camera", {
      position: [...camera.position],
      target: [...camera.target],
      up: [...camera.up],
    });
    this.#model.save_changes();
  }

  publishInteraction(event: InteractionEvent): void {
    // Complete engine identity is preserved. A pick is not a selection query.
    this.#model.set("interaction_event", { ...event });
    this.#model.set(
      "pick",
      event.kind === "pick" && event.result !== undefined
        ? (JSON.parse(event.result) as Record<string, unknown>)
        : {},
    );
    this.#model.save_changes();
  }
}
