import type {
  InlineRuntimeSource,
  InteractionEvent,
  SceneSnapshot,
  SceneSource,
  ViewerSink,
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
