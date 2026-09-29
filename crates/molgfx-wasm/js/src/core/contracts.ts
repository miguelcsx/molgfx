// The seam between the reusable viewer core and whatever hosts it -- the
// anywidget/Jupyter adapter today, a plain-page embed later. Core code only
// ever depends on these; a host implements them however its transport works.
import type { BinaryState, Cleanup, Camera } from "./types.js";
import type { InteractionModifiers } from "./interaction.js";
export interface StructureSource {
  id: bigint;
  name: string;
  payload: BinaryState;
}

/**
 * A scene the host declares (`spec` plus the sources it binds), or one local
 * file the runtime parses once and turns into a scene of its own.
 */
export type SceneSnapshot =
  | { spec: string; structures: StructureSource[] }
  | { file: { name: string; payload: BinaryState }; structures?: never };

/** What to render, and how the host tells the core it changed. */
export interface SceneSource {
  /** The scene to (re)build. Read fresh each call -- never cached by the caller. */
  snapshot(): SceneSnapshot;
  /** The revision the host claims to be at; a host that owns no copy of the scene omits it. */
  revision?(): bigint;
  /** A full scene replacement is available; the core re-reads `snapshot()`. */
  onReplace(callback: () => void): Cleanup;
  /** One incremental patch, encoded exactly as the wasm runtime expects it. */
  onPatch(callback: (patchJson: string) => void): Cleanup;
  /** The core's scene fell behind (or started behind); ask the host to catch it up. */
  requestResync(): void;
}

/** A pick result plus the input context captured before asynchronous readback. */
export type InteractionEvent =
  | {
      readonly kind: "pick";
      /** Raw native pick payload; not an atom identity by itself. */
      readonly result?: string;
      /** Monotonic identity for host-side deduplication of transport events. */
      readonly eventId: string;
      readonly modifiers: InteractionModifiers;
      readonly sceneGeneration: number;
    }
  | { readonly kind: "clear"; readonly eventId: string; readonly sceneGeneration: number };

/** Where the core reports back out: camera moves, picks, and failures. */
export interface ViewerSink {
  reportError(error: unknown): void;
  publishCamera(camera: Camera): void;
  /** Legacy pick projection retained for existing hosts. */
  publishPick(result: string | undefined): void;
  /** Typed transport is authoritative; raw pick projection is diagnostic only. */
  publishInteraction(event: InteractionEvent): void;
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
