// A host with no kernel: the page itself holds the structure file, and the
// viewer core parses it once into the only scene there is.
import type { InteractionEvent, SceneSnapshot, SceneSource, ViewerSink } from "../core/contracts.js";
import type { BinaryState, Camera, Cleanup } from "../core/types.js";

export interface LocalHostEvents {
  onError?(error: unknown): void;
  onPick?(result: string | undefined): void;
}

/** One local structure file as a `SceneSource`, and the viewer's reports as callbacks. */
export class LocalHost implements SceneSource, ViewerSink {
  readonly #replaced = new Set<() => void>();
  readonly #events: LocalHostEvents;
  #file: { name: string; payload: BinaryState };

  constructor(name: string, payload: BinaryState, events: LocalHostEvents = {}) {
    this.#file = { name, payload };
    this.#events = events;
  }

  /** Replace the structure; the viewer rebuilds its scene from the new file. */
  load(name: string, payload: BinaryState): void {
    this.#file = { name, payload };
    for (const callback of this.#replaced) callback();
  }

  snapshot(): SceneSnapshot { return { file: this.#file }; }

  onReplace(callback: () => void): Cleanup {
    this.#replaced.add(callback);
    return () => { this.#replaced.delete(callback); };
  }

  // Edits happen in place through the console; nothing arrives as a patch.
  onPatch(): Cleanup { return () => {}; }
  requestResync(): void {}

  reportError(error: unknown): void { this.#events.onError?.(error); }
  publishCamera(_camera: Camera): void {}
  publishPick(result: string | undefined): void { this.#events.onPick?.(result); }
  publishInteraction(_event: InteractionEvent): void {}
}
