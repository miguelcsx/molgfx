import type { BinaryState, Camera, Cleanup } from "../core/transport.js";
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
  structure_sources?: Array<number | string | bigint>;
  sync_request: number;
  error: string;
  camera: Camera;
  pick: Record<string, unknown>;
  interaction_event: Record<string, unknown>;
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
export function observeModel(
  model: WidgetModel,
  event: ModelChangeEvent,
  callback: () => void,
): Cleanup {
  model.on(event, callback);
  return () => model.off(event, callback);
}
