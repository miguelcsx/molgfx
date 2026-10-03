export type StructureBytes = ArrayBuffer | ArrayBufferView;
export type StructureInput = File | StructureBytes;
export type Vec3 = [number, number, number];
export interface CameraState {
  position: Vec3;
  target: Vec3;
  up: Vec3;
}
export interface SelectionModifiers {
  readonly shift: boolean;
  readonly control: boolean;
  readonly meta: boolean;
  readonly alt: boolean;
}
/** Semantic identity supplied by the engine; never reconstructed from labels. */
export interface Pick {
  readonly pick:
    "atom" | "bond" | "label" | "measurement" | "volume_segment" | "non_atom";
  readonly [key: string]: unknown;
}
export interface PickEvent {
  readonly result: Pick | undefined;
  readonly modifiers: SelectionModifiers;
}
export interface SelectionChangeEvent {
  readonly selection: string;
}
export interface CommandResult {
  readonly revision: number;
  readonly messages: readonly string[];
}
export interface ShowOptions {
  name?: string;
}
export interface LoadOptions {
  name?: string;
}
export interface ViewerEvents {
  pick: PickEvent;
  selectionchange: SelectionChangeEvent;
  error: Error;
  camera: CameraState;
}
export interface ViewerOptions {
  structure?: StructureInput;
  name?: string;
  signal?: AbortSignal;
  onPick?: (event: PickEvent) => void;
  onSelectionChange?: (event: SelectionChangeEvent) => void;
  onError?: (error: Error) => void;
  onCamera?: (camera: CameraState) => void;
}
