/** Typed interaction values shared by input adapters and the viewer core. */
export interface InteractionModifiers {
  readonly shift: boolean;
  readonly control: boolean;
  readonly meta: boolean;
  readonly alt: boolean;
}

export interface PickRequest {
  readonly x: number;
  readonly y: number;
  readonly modifiers: InteractionModifiers;
  /** Captured before the asynchronous readback starts. */
  readonly sceneGeneration: number;
}

export interface HoverRequest {
  readonly x: number;
  readonly y: number;
  /** Hover is intentionally local and latest-only. */
  readonly token: number;
}

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
