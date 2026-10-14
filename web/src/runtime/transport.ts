import type {
  TimeChange,
  TransportEvents,
  TransportLoopMode,
  TransportOptions,
} from "../core/contracts.js";
import { ReadoutBuffer } from "./timeline-readout.js";
type Listener<K extends keyof TransportEvents> = (
  event: TransportEvents[K],
) => void;
/**
 * Headless playback over presentation seconds. It owns the clock position, the
 * loop policy and the tick; it draws nothing and has no controls. Each tick
 * applies the WASM timeline and reads its tracks back into reused storage.
 */
export class Transport<Scene = never> {
  readonly #options: TransportOptions<Scene>;
  readonly #buffer: ReadoutBuffer;
  readonly #change: { playbackSeconds: number; readouts: ReadoutBuffer["readouts"] };
  readonly #listeners = {
    timechange: new Set<Listener<"timechange">>(),
    playingchange: new Set<Listener<"playingchange">>(),
  };
  readonly #tick = (now: number): void => this.#advance(now);
  #mode: TransportLoopMode;
  #rate: number;
  /** Unfolded position: pingpong keeps it in [0, 2 * duration) and folds for display. */
  #cursor = 0;
  #playing = false;
  #handle: number | undefined;
  #last: number | undefined;
  #disposed = false;
  constructor(options: TransportOptions<Scene>) {
    if (!(options.durationSeconds > 0) || !Number.isFinite(options.durationSeconds))
      throw new RangeError("durationSeconds must be finite and positive");
    this.#options = options;
    this.#mode = options.loopMode ?? "clamp";
    this.#rate = options.rate ?? 1;
    this.#buffer = new ReadoutBuffer(options.layout, options.timeline.trackCount);
    this.#change = { playbackSeconds: 0, readouts: this.#buffer.readouts };
  }
  get playing(): boolean {
    return this.#playing;
  }
  get rate(): number {
    return this.#rate;
  }
  get loopMode(): TransportLoopMode {
    return this.#mode;
  }
  get durationSeconds(): number {
    return this.#options.durationSeconds;
  }
  /** Playback seconds inside [0, duration] after the loop policy. */
  get playbackSeconds(): number {
    return this.#fold(this.#cursor);
  }
  on<K extends keyof TransportEvents>(type: K, listener: Listener<K>): () => void {
    const set = this.#listeners[type] as Set<Listener<K>>;
    set.add(listener);
    return () => {
      set.delete(listener);
    };
  }
  set rate(value: number) {
    if (!Number.isFinite(value)) throw new RangeError("rate must be finite");
    this.#rate = value;
  }
  set loopMode(mode: TransportLoopMode) {
    const shown = this.playbackSeconds;
    this.#mode = mode;
    this.#cursor = shown;
  }
  play(): void {
    if (this.#disposed || this.#playing) return;
    this.#playing = true;
    this.#last = undefined;
    this.#emitPlaying();
    this.#schedule();
  }
  pause(): void {
    if (!this.#playing) return;
    this.#playing = false;
    this.#cancel();
    this.#emitPlaying();
  }
  /** Moves to `seconds` (clamped to the range), applies once and emits `timechange`. */
  seek(seconds: number): void {
    if (this.#disposed) return;
    if (Number.isNaN(seconds)) throw new RangeError("seek target must be a number");
    this.#cursor = Math.min(Math.max(seconds, 0), this.#options.durationSeconds);
    this.#publish();
  }
  dispose(): void {
    if (this.#disposed) return;
    this.#playing = false;
    this.#cancel();
    this.#disposed = true;
    this.#listeners.timechange.clear();
    this.#listeners.playingchange.clear();
  }
  #advance(now: number): void {
    this.#handle = undefined;
    if (!this.#playing) return;
    const previous = this.#last;
    this.#last = now;
    if (previous !== undefined && this.#rate !== 0) {
      this.#step(((now - previous) / 1000) * this.#rate);
      this.#publish();
    }
    if (this.#playing) this.#schedule();
  }
  #step(delta: number): void {
    const duration = this.#options.durationSeconds;
    const next = this.#cursor + delta;
    if (this.#mode === "clamp") {
      this.#cursor = Math.min(Math.max(next, 0), duration);
      if (this.#cursor !== next) this.#stopAtBoundary();
      return;
    }
    const period = this.#mode === "loop" ? duration : 2 * duration;
    const wrapped = next % period;
    this.#cursor = wrapped < 0 ? wrapped + period : wrapped;
  }
  /** Clamp mode ends playback at a boundary so the render loop can stop. */
  #stopAtBoundary(): void {
    this.#playing = false;
    this.#emitPlaying();
  }
  #fold(cursor: number): number {
    const duration = this.#options.durationSeconds;
    if (this.#mode !== "pingpong" || cursor <= duration) return cursor;
    return 2 * duration - cursor;
  }
  #publish(): void {
    const seconds = this.playbackSeconds;
    this.#options.timeline.apply(this.#options.scene as never, seconds);
    this.#buffer.decode(this.#options.timeline.readingsInto(this.#buffer.raw));
    this.#change.playbackSeconds = seconds;
    const payload: TimeChange = this.#change;
    for (const listener of this.#listeners.timechange) listener(payload);
  }
  #emitPlaying(): void {
    for (const listener of this.#listeners.playingchange) listener(this.#playing);
  }
  #schedule(): void {
    if (this.#handle === undefined)
      this.#handle = this.#options.clock.request(this.#tick);
  }
  #cancel(): void {
    if (this.#handle !== undefined) this.#options.clock.cancel(this.#handle);
    this.#handle = undefined;
  }
}
