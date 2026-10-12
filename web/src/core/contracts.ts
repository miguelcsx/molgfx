import type { BinaryState, Cleanup, Camera } from "./transport.js";
import type { InteractionModifiers } from "./interaction.js";
export interface StructureSource {
  id: bigint;
  name: string;
  payload: BinaryState;
}
export interface ScientificBindings {
  coordinateFrames?: CoordinateFrame[];
  trajectories?: Array<{ id: bigint; start: bigint; end: bigint; seconds: number }>;
  playback?: Array<{ structure: bigint; start: bigint; end: bigint; seconds: number }>;
  volumes: Array<{ id: bigint; values: BinaryState }>;
  hydrogenBonds: Array<{ id: bigint; endpoints: BinaryState; candidateDistance?: number; pose?: string }>;
  molecularSurfaces: Array<{ id: bigint; metadata: string; vertices: BinaryState; normals: BinaryState; faces: BinaryState; atoms: BinaryState }>;
}
export interface CoordinateFrame {
  id: bigint;
  index: bigint;
  seconds: number;
  positions: BinaryState;
}
export interface PlaybackUpdate {
  structure: bigint;
  start: CoordinateFrame;
  end: CoordinateFrame;
  seconds: number;
}
export type SceneSnapshot = { spec: string; structures: StructureSource[]; bindings?: ScientificBindings };
/** A remote Session owns this scene; the host transports committed state only. */
export interface SceneSource {
  snapshot(): SceneSnapshot;
  revision(): bigint;
  onReplace(callback: () => void): Cleanup;
  onTrajectorySample?(callback: (structure: bigint, seconds: number) => void): Cleanup;
  onTrajectoryPair?(callback: (update: PlaybackUpdate) => void): Cleanup;
  onPatch(callback: (patchJson: string) => void): Cleanup;
  requestResync(): void;
}
export type InteractionEvent =
  | {
      readonly kind: "pick";
      readonly result?: string;
      readonly eventId: string;
      readonly modifiers: InteractionModifiers;
      readonly sceneGeneration: number;
    }
  | {
      readonly kind: "clear";
      readonly eventId: string;
      readonly sceneGeneration: number;
    };
export interface ViewerSink {
  reportError(error: unknown): void;
  publishCamera(camera: Camera): void;
  publishInteraction(event: InteractionEvent): void;
}
export interface InlineRuntimeSource {
  glue(): string | undefined;
  key(): string | undefined;
  wasmGzip(): BinaryState | undefined;
}
export interface ViewerHost {
  el: HTMLElement;
  source: SceneSource;
  sink: ViewerSink;
  inlineRuntime?: InlineRuntimeSource;
  signal?: AbortSignal;
}
