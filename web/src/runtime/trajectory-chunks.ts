import type { ChunkRequest, CoordinateFrame } from "../core/contracts.js";

/** Frames the kernel sends per chunk. */
export const CHUNK_FRAMES = 8;
/** Prefetch depth: the window keeps this many chunks, the one in view plus one ahead. */
export const CHUNKS_AHEAD = 2;
/** Hard cap on frames held per trajectory, beyond the two the scene keeps resident. */
export const CHUNK_WINDOW_FRAMES = CHUNK_FRAMES * CHUNKS_AHEAD;

/** A request before the stream names its structure. */
export type WindowRequest = Omit<ChunkRequest, "structure">;

export interface FramePair {
  start: CoordinateFrame;
  end: CoordinateFrame;
}

/**
 * A bounded, index-ordered window over one trajectory. It holds at most
 * `CHUNK_WINDOW_FRAMES` frames, asks for the next chunk before the current one
 * runs out, and never has two requests in flight.
 */
export class ChunkWindow {
  #frames: CoordinateFrame[] = [];
  #pending = false;
  #exhausted = false;

  get size(): number {
    return this.#frames.length;
  }

  /** The two frames whose times bracket `seconds`, if the window covers it. */
  pairAt(seconds: number): FramePair | undefined {
    const at = this.#bracket(seconds);
    const start = this.#frames[at];
    const end = this.#frames[at + 1];
    return at < 0 || !start || !end ? undefined : { start, end };
  }

  /** The request needed to keep `seconds` covered, or undefined when none is due. */
  next(seconds: number): WindowRequest | undefined {
    if (this.#pending) return undefined;
    const last = this.#frames.at(-1);
    const first = this.#frames[0];
    if (!first || !last || seconds < first.seconds) return this.#ask(seconds);
    if (seconds > last.seconds) return this.#exhausted ? undefined : this.#ask(seconds);
    if (this.#exhausted) return undefined;
    const ahead = this.#frames.length - 1 - Math.max(this.#bracket(seconds), 0);
    return ahead < CHUNK_FRAMES ? this.#ask(last.seconds) : undefined;
  }

  /** Merges a reply that touches the window, otherwise replaces the window with it. */
  accept(frames: readonly CoordinateFrame[], seconds: number): void {
    this.#pending = false;
    if (frames.length === 0) {
      this.#exhausted = true;
      return;
    }
    const known = new Set(this.#frames.map((frame) => frame.index));
    const incoming = frames.filter((frame) => !known.has(frame.index));
    const last = this.#frames.at(-1);
    const first = this.#frames[0];
    const head = frames[0];
    const tail = frames.at(-1);
    const touches =
      last && first && head && tail && head.index <= last.index + 1n && tail.index >= first.index - 1n;
    if (touches) {
      this.#exhausted = incoming.length === 0 && tail.index >= last.index;
      this.#frames = [...this.#frames, ...incoming];
    } else {
      this.#exhausted = false;
      this.#frames = [...frames];
    }
    this.#frames.sort((a, b) => (a.index < b.index ? -1 : a.index > b.index ? 1 : 0));
    this.#trim(seconds);
  }

  clear(): void {
    this.#frames = [];
    this.#pending = false;
    this.#exhausted = false;
  }

  #ask(fromSeconds: number): WindowRequest {
    this.#pending = true;
    return { fromSeconds, count: CHUNK_FRAMES };
  }

  /** Position of the last frame at or before `seconds`; -1 when the window starts later. */
  #bracket(seconds: number): number {
    let at = -1;
    for (let i = 0; i < this.#frames.length; i += 1) {
      if ((this.#frames[i]?.seconds ?? Infinity) <= seconds) at = i;
      else break;
    }
    const end = this.#frames.length - 1;
    return at === end && at > 0 ? end - 1 : at;
  }

  /** Keeps the frame behind the current bracket so a small step back needs no request. */
  #trim(seconds: number): void {
    const excess = this.#frames.length - CHUNK_WINDOW_FRAMES;
    if (excess <= 0) return;
    const drop = Math.min(Math.max(this.#bracket(seconds) - 1, 0), excess);
    this.#frames.splice(0, drop);
    this.#frames.length = Math.min(this.#frames.length, CHUNK_WINDOW_FRAMES);
  }
}
