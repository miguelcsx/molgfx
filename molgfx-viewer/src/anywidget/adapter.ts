import type {
  InlineRuntimeSource,
  InteractionEvent,
  SceneSnapshot,
  SceneSource,
  ScienceSink,
  ViewerSink,
} from "../core/contracts.js";
import type {
  Camera,
  Cleanup,
  MeasurementRequest,
  TrajectoryFrameInput,
  VolumeSigmaControls,
} from "../core/types.js";
import { observeModel } from "./model.js";
import type { WidgetModel } from "./model.js";

/** Adapts one anywidget `WidgetModel` to every port the viewer core needs. */
export class AnywidgetAdapter implements SceneSource, InlineRuntimeSource, ViewerSink {
  readonly #model: WidgetModel;
  readonly science: ScienceSink = this;

  constructor(model: WidgetModel) {
    this.#model = model;
  }

  snapshot(): SceneSnapshot {
    const model = this.#model;
    const ids = model.get("structure_ids");
    const names = model.get("structure_names");
    const payloads = model.get("structure_payloads");
    if (ids.length !== names.length || ids.length !== payloads.length) {
      throw new Error("structure transport columns have different lengths");
    }
    return {
      spec: model.get("scene_spec"),
      structures: ids.map((id, index) => {
        const payload = payloads[index];
        if (id === undefined || payload === undefined) {
          throw new Error(`missing structure transport value at index ${index}`);
        }
        return { id: BigInt(id), name: names[index] ?? "", payload };
      }),
    };
  }

  revision(): bigint { return BigInt(this.#model.get("revision")); }

  onReplace(callback: () => void): Cleanup {
    return observeModel(this.#model, "change:scene_spec", callback);
  }

  onPatch(callback: (patchJson: string) => void): Cleanup {
    return observeModel(this.#model, "change:patch_sequence", () => callback(this.#model.get("scene_patch")));
  }

  requestResync(): void {
    this.#model.set("sync_request", (this.#model.get("sync_request") || 0) + 1);
    this.#model.save_changes();
  }

  glue(): string | undefined { return this.#model.get("_runtime_js"); }
  key(): string | undefined { return this.#model.get("_runtime_key"); }
  wasmGzip() { return this.#model.get("_runtime_wasm"); }

  reportError(error: unknown): void {
    this.#model.set("error", error instanceof Error ? error.message : String(error));
    this.#model.save_changes();
  }

  publishCamera(camera: Camera): void {
    this.#model.set("camera", { position: [...camera.position], target: [...camera.target], up: [...camera.up] });
    this.#model.save_changes();
  }

  /** Legacy pick/selection traits: a projection of the authoritative event. */
  #publishLegacyPick(result: string | undefined): void {
    if (result === undefined) {
      this.#model.set("pick", {});
      this.#model.set("selection", "");
      return;
    }
    // Native payloads are the renderer's fixed PickResult schema, but a
    // malformed body must fail loudly instead of corrupting widget state.
    this.#model.set("pick", JSON.parse(result) as Record<string, unknown>);
    this.#model.set("selection", result);
  }

  publishPick(result: string | undefined): void {
    this.#publishLegacyPick(result);
    this.#model.save_changes();
  }

  publishInteraction(event: InteractionEvent): void {
    const wire = wireObject(event);
    this.#model.set("interaction_event", wire);
    this.#model.set("interaction", wire);
    // Only pick events carry a semantic result to project; clears clear it.
    this.#publishLegacyPick(event.kind === "pick" ? event.result : undefined);
    this.#model.save_changes();
  }

  publishSequenceIntervals(intervals: readonly [number, number][], chain: string, structure: bigint): void {
    this.#model.set("sequence_intervals", { intervals, chain, structure: structure.toString() });
    this.#model.save_changes();
  }

  publishFocusPreset(preset: "ligand" | "selection"): void {
    this.#model.set("focus_preset", preset);
    this.#model.save_changes();
  }

  commitMeasurement(request: MeasurementRequest): void {
    this.#model.set("measurement_request", wireObject(request));
    this.#model.save_changes();
  }

  setVolumeSigma(volume: bigint, controls: VolumeSigmaControls): void {
    this.#model.set("volume_sigma", { volume: volume.toString(), ...controls });
    this.#model.save_changes();
  }

  bindTrajectoryFrame(frame: TrajectoryFrameInput): void {
    this.#model.set("trajectory_frame", wireObject(frame));
    this.#model.save_changes();
  }

  setTrajectoryTime(structure: bigint, seconds: number, topologyRevision: bigint): void {
    this.#model.set("trajectory_time", {
      structure: structure.toString(), seconds, topology_revision: topologyRevision.toString(),
    });
    this.#model.save_changes();
  }
}

/** Convert bigint identities to JSON-safe wire values without copying binary payloads. */
function wireObject(value: object): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, entry] of Object.entries(value)) result[key] = wireValue(entry);
  return result;
}

function wireValue(value: unknown): unknown {
  if (typeof value === "bigint") return value.toString();
  if (Array.isArray(value)) return value.map(wireValue);
  if (value instanceof ArrayBuffer || ArrayBuffer.isView(value)) return value;
  if (value && typeof value === "object") return wireObject(value);
  return value;
}
