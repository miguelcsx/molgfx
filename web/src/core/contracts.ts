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
/** Consecutive frames of one trajectory, in ascending index order. */
export interface TrajectoryChunk {
  structure: bigint;
  frames: CoordinateFrame[];
}
/** Asks for `count` frames starting at the last frame at or before `fromSeconds`. */
export interface ChunkRequest {
  structure: bigint;
  fromSeconds: number;
  count: number;
}
export type SceneSnapshot = { spec: string; structures: StructureSource[]; bindings?: ScientificBindings };
/** A remote Session owns this scene; the host transports committed state only. */
export interface SceneSource {
  snapshot(): SceneSnapshot;
  revision(): bigint;
  onReplace(callback: () => void): Cleanup;
  onTrajectorySample?(callback: (structure: bigint, seconds: number) => void): Cleanup;
  onTrajectoryPair?(callback: (update: PlaybackUpdate) => void): Cleanup;
  /** A window of consecutive frames the kernel sent in answer to `requestTrajectoryChunk`. */
  onTrajectoryChunk?(callback: (chunk: TrajectoryChunk) => void): Cleanup;
  requestTrajectoryChunk?(request: ChunkRequest): void;
  /** Lets the kernel drive the transport and receive its `timechange` events. */
  bindTransport?(transport: TransportPort): Cleanup;
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
/** How playback behaves at either end of the presentation range. */
export type TransportLoopMode = "clamp" | "loop" | "pingpong";
export type TransportTimeUnit = "as" | "fs" | "ps" | "ns" | "us" | "ms" | "s";
/** One track's typed time at the most recently applied playback second. */
export interface TrackReadout {
  track: number;
  /** Physical time in canonical seconds. */
  seconds: number;
  /** Fraction of the track's interval already played, in [0, 1]. */
  phase: number;
  unit: TransportTimeUnit;
  /** Physical time expressed in `unit`. */
  value: number;
}
/**
 * Payload of `timechange`. The object and its readouts are reused by the
 * transport on every tick, so read them inside the handler and copy what must
 * outlive it.
 */
export interface TimeChange {
  playbackSeconds: number;
  readouts: readonly TrackReadout[];
}
export interface TransportEvents {
  timechange: TimeChange;
  playingchange: boolean;
}
/** The subset of the WASM `Timeline` the transport drives. */
export interface TransportTimeline {
  readonly trackCount: number;
  apply(scene: never, playbackSeconds: number): void;
  readingsInto(out: Float64Array): number;
}
/** `Timeline.readingStride()` and `Timeline.unitSymbols()`, resolved once from the runtime class. */
export interface TimelineLayout {
  readingStride: number;
  unitSymbols: readonly string[];
}
/** A requestAnimationFrame-style scheduler; the transport never touches the global one. */
export interface TransportClock {
  request(callback: (nowMilliseconds: number) => void): number;
  cancel(handle: number): void;
}
export interface TransportOptions<Scene = never> {
  timeline: TransportTimeline;
  layout: TimelineLayout;
  scene: Scene;
  clock: TransportClock;
  /** Length of the presentation range, in playback seconds. */
  durationSeconds: number;
  loopMode?: TransportLoopMode;
  /** Signed playback seconds per wall-clock second; zero holds. Defaults to 1. */
  rate?: number;
}
/** What a kernel or host may do to a transport; `Transport` implements it. */
export interface TransportPort {
  play(): void;
  pause(): void;
  seek(seconds: number): void;
  rate: number;
  loopMode: TransportLoopMode;
  on<K extends keyof TransportEvents>(
    type: K,
    listener: (event: TransportEvents[K]) => void,
  ): () => void;
}
/** One structure's physical interval, fitted across the whole presentation range. */
export interface TransportTrack {
  structure: bigint;
  /** `[start, end]` in canonical seconds, inside the structure's resident frame pair. */
  range: readonly [number, number];
  mode?: TransportLoopMode;
  unit?: TransportTimeUnit;
}
export interface TransportSetup {
  durationSeconds: number;
  tracks: readonly TransportTrack[];
  loopMode?: TransportLoopMode;
  rate?: number;
}
/**
 * Options of `viewer.captureSequence`. Frame `i` is rendered at playback
 * seconds exactly `i / fps`, converged, with no dependence on animation-frame
 * timing. Frames are handed over one at a time through `onFrame`, which the
 * capture awaits before it renders the next one, so only one frame is in flight.
 */
export interface SequenceOptions {
  /** Positive integer frames per playback second. */
  fps: number;
  /** Playback seconds to cover, from 0; defaults to the transport duration and may not exceed it. */
  seconds?: number;
  /** Pixel size of every frame; the canvas is restored afterwards. */
  size: { width: number; height: number };
  /** Receives each frame in order. Awaited, so backpressure is the caller's pace. */
  onFrame(frame: SequenceFrame): void | Promise<void>;
  /** Stops the sequence between frames; the transport stays paused. */
  signal?: AbortSignal;
}
/**
 * One captured frame. The object and its readouts are reused for every frame:
 * read them inside `onFrame` and copy what must outlive it. The image is
 * the caller's to keep.
 */
export interface SequenceFrame {
  index: number;
  /** Exactly `index / fps`. */
  playbackSeconds: number;
  /** PNG of the canvas at this frame. */
  image: Blob;
  readouts: readonly TrackReadout[];
}
export interface SequenceTrackTime {
  track: number;
  /** Physical time in canonical seconds, taken from file timestamps. */
  simSeconds: number;
  unit: TransportTimeUnit;
}
export interface SequenceManifestFrame {
  index: number;
  playbackSeconds: number;
  /** False when the frame hit the pass cap before the renderer finished accumulating; it is still delivered. */
  converged: boolean;
  tracks: SequenceTrackTime[];
}
/** The sidecar of a capture, built in memory and returned; nothing is written to disk. */
export interface SequenceManifest {
  fps: number;
  frameCount: number;
  /** False when the abort signal stopped the sequence before every frame was delivered. A failed capture throws instead of returning. */
  completed: boolean;
  /** How many delivered frames have `converged: false`. */
  unconvergedFrames: number;
  width: number;
  height: number;
  interpolation: string;
  frames: SequenceManifestFrame[];
}
