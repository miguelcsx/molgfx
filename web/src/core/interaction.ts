import type { SelectionModifiers as InteractionModifiers } from "../types.js";
export type { SelectionModifiers as InteractionModifiers } from "../types.js";

export function interactionModifiers(event: MouseEvent): InteractionModifiers {
  return {
    shift: event.shiftKey,
    control: event.ctrlKey,
    meta: event.metaKey,
    alt: event.altKey,
  };
}

/** Monotonic guard for asynchronous picks crossing scene replacement. */
export class SceneGeneration {
  #value = 0;

  get value(): number {
    return this.#value;
  }

  advance(): number {
    this.#value += 1;
    return this.#value;
  }

  isCurrent(value: number): boolean {
    return value === this.#value;
  }
}
