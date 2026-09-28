// The traitlets sync contract with the Python kernel (see
// python/molgfx/viewer/viewer.py and workbench.py). Nothing outside this
// adapter should import from here -- the viewer core knows only
// core/contracts.ts.
import type { CommandReply, CommandRequest } from "../core/contracts.js";
import type { BinaryState, Camera, Cleanup } from "../core/types.js";

export interface WidgetTraits {
  _runtime_js?: string;
  _runtime_key?: string;
  _runtime_wasm?: BinaryState;

  scene_spec: string;
  scene_patch: string;
  patch_sequence: number;
  revision: number | string | bigint;

  structure_ids: Array<number | string | bigint>;
  structure_names: Array<string | null | undefined>;
  structure_payloads: BinaryState[];

  sync_request: number;
  error: string;

  camera: Camera;

  /** Legacy diagnostic projections retained for existing notebook consumers. */
  pick: Record<string, unknown>;
  selection: string;
  interaction: Record<string, unknown>;
  /** Typed interaction/science transports sent to the kernel. */
  interaction_event: Record<string, unknown>;
  sequence_intervals: Record<string, unknown>;
  focus_preset: string;
  measurement_request: Record<string, unknown>;
  volume_sigma: Record<string, unknown>;
  trajectory_frame: Record<string, unknown>;
  trajectory_time: Record<string, unknown>;

  workbench: boolean;

  command_request: CommandRequest;
  command_reply: CommandReply | Record<string, never>;

  history: string[];
}

export type ModelChangeEvent = `change:${keyof WidgetTraits & string}`;

export interface WidgetModel {
  get<K extends keyof WidgetTraits>(key: K): WidgetTraits[K];
  set<K extends keyof WidgetTraits>(key: K, value: WidgetTraits[K]): void;
  save_changes(): void;
  on(event: ModelChangeEvent, callback: () => void): void;
  off(event: ModelChangeEvent, callback: () => void): void;
}

export interface RenderContext {
  model: WidgetModel;
  el: HTMLElement;
}

export function observeModel(model: WidgetModel, event: ModelChangeEvent, callback: () => void): Cleanup {
  model.on(event, callback);
  return () => model.off(event, callback);
}
