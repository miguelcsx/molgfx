import type { ViewerEvents } from "../types.js";
export class ViewerEmitter {
  readonly #listeners = new Map<
    keyof ViewerEvents,
    Set<(event: never) => void>
  >();
  #disposed = false;
  on<K extends keyof ViewerEvents>(
    type: K,
    listener: (event: ViewerEvents[K]) => void,
  ): () => void {
    if (this.#disposed) throw new Error("Viewer is disposed");
    let listeners = this.#listeners.get(type);
    if (!listeners) {
      listeners = new Set();
      this.#listeners.set(type, listeners);
    }
    listeners.add(listener as (event: never) => void);
    return () => {
      listeners.delete(listener as (event: never) => void);
    };
  }
  emit<K extends keyof ViewerEvents>(type: K, event: ViewerEvents[K]): void {
    for (const listener of this.#listeners.get(type) ?? []) {
      try {
        listener(event as never);
      } catch (error) {
        queueMicrotask(() => {
          throw error;
        });
      }
    }
  }
  dispose(): void {
    this.#disposed = true;
    for (const listeners of this.#listeners.values()) listeners.clear();
    this.#listeners.clear();
  }
}
