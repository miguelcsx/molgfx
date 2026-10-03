import type { Cleanup } from "./transport.js";

export class CleanupBag {
  readonly #items: Cleanup[] = [];
  #disposed = false;

  add(cleanup: Cleanup): Cleanup {
    if (this.#disposed) {
      cleanup();
      return cleanup;
    }

    this.#items.push(cleanup);
    return cleanup;
  }

  dispose(): void {
    if (this.#disposed) {
      return;
    }

    this.#disposed = true;

    for (let index = this.#items.length - 1; index >= 0; index -= 1) {
      this.#items[index]?.();
    }

    this.#items.length = 0;
  }
}

export function listen<K extends keyof GlobalEventHandlersEventMap>(
  target: EventTarget,
  type: K,
  listener: (event: GlobalEventHandlersEventMap[K]) => void,
  options?: boolean | AddEventListenerOptions,
): Cleanup {
  const eventListener = listener as EventListener;
  target.addEventListener(type, eventListener, options);
  return () => target.removeEventListener(type, eventListener, options);
}
