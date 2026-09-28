// The seam between the reusable viewer core and whatever hosts it -- the
// anywidget/Jupyter adapter today, a plain-page embed later. Core code only
// ever depends on these; a host implements them however its transport works.
import type {
  BinaryState,
  Cleanup,
  MeasurementRequest,
  VolumeSigmaControls,
  TrajectoryFrameInput,
  Camera,
  ExactAtom,
} from "./types.js";
import type { InteractionModifiers } from "./interaction.js";
/** Optional sequence-panel actions supplied by the host transport. */
export interface SequenceSink {
  publishSequenceIntervals?(
    intervals: readonly [number, number][],
    chain: string,
    structure: bigint,
  ): void;
  publishFocusPreset?(preset: "ligand" | "selection"): void;
}

export interface StructureSource {
  id: bigint;
  name: string;
  payload: BinaryState;
}

export interface SceneSnapshot {
  spec: string;
  structures: StructureSource[];
}

/** What to render, and how the host tells the core it changed. */
export interface SceneSource {
  /** The scene to (re)build. Read fresh each call -- never cached by the caller. */
  snapshot(): SceneSnapshot;
  /** The revision the host currently claims to be at. */
  revision(): bigint;
  /** A full scene replacement is available; the core re-reads `snapshot()`. */
  onReplace(callback: () => void): Cleanup;
  /** One incremental patch, encoded exactly as the wasm runtime expects it. */
  onPatch(callback: (patchJson: string) => void): Cleanup;
  /** The core's scene fell behind (or started behind); ask the host to catch it up. */
  requestResync(): void;
}

/** Native exact-atom identity, when the renderer binding supplies one. */
export interface ExactAtomPick {
  readonly atom: ExactAtom;
  readonly raw?: string;
}

/** A pick result plus the input context captured before asynchronous readback. */
export type InteractionEvent =
  | {
      readonly kind: "pick";
      /** Raw native pick payload; not an atom identity by itself. */
      readonly result?: string;
      /** Optional typed identity supplied by a native exact-atom binding. */
      readonly exactAtom?: ExactAtomPick;
      /** Monotonic identity for host-side deduplication of transport events. */
      readonly eventId: string;
      readonly modifiers: InteractionModifiers;
      readonly sceneGeneration: number;
    }
  | { readonly kind: "clear"; readonly eventId: string; readonly sceneGeneration: number };

/** Durable scientific operations exposed by a host transport. */
export interface ScienceSink {
  /** Commit only after all atom identities still belong to the current topology. */
  commitMeasurement(request: MeasurementRequest): void;
  /** Update map contour controls using the map's recorded statistics. */
  setVolumeSigma(volume: bigint, controls: VolumeSigmaControls): void;
  /** Replace the resident trajectory interval for a structure. */
  bindTrajectoryFrame(frame: TrajectoryFrameInput): void;
  /** Apply the latest coalesced sample time to a resident interval. */
  setTrajectoryTime(structure: bigint, seconds: number, topologyRevision: bigint): void;
}

/** Where the core reports back out: camera moves, picks, and failures. */
export interface ViewerSink extends SequenceSink {
  reportError(error: unknown): void;
  publishCamera(camera: Camera): void;
  /** Legacy pick projection retained for existing hosts. */
  publishPick(result: string | undefined): void;
  /** Typed transport is authoritative; raw pick projection is diagnostic only. */
  publishInteraction(event: InteractionEvent): void;
  /** Scientific operations share the host's durable transaction boundary. */
  science: ScienceSink;
}

/**
 * A kernel that can reach this page may push the wasm bindings inline instead
 * of the core importing them as a static asset -- see `runtime/runtime.ts`.
 * A host with no such channel simply has nothing that implements this.
 */
export interface InlineRuntimeSource {
  glue(): string | undefined;
  key(): string | undefined;
  wasmGzip(): BinaryState | undefined;
}

export interface CompletionItem {
  text: string;
  detail?: string;
  kind?: string;
}

export interface CommandError {
  message?: string;
}

export type CommandRequest =
  | { id: string; type: "execute"; text: string }
  | { id: string; type: "complete"; text: string; cursor: number };

export type CommandReply =
  | { id: string; type: "completions"; items?: CompletionItem[] }
  | { id: string; type: "result"; ok: true; text: string; revision: number; messages?: string[] }
  | { id: string; type: "result"; ok: false; text: string; errors?: CommandError[]; rendered?: string };

/** The optional command console's backend: submit a request, hear its reply. */
export interface CommandBackend {
  readonly history: string[];
  submit(request: CommandRequest): void;
  onReply(callback: (reply: CommandReply) => void): Cleanup;
}
