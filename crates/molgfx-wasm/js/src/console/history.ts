import type { CommandBackend } from "../core/contracts.js";

/** Recall through submitted command history, restoring the in-progress draft at its end. */
export class HistoryNavigator {
  readonly #input: HTMLInputElement;
  readonly #backend: CommandBackend;

  #index = -1;
  #draft = "";

  constructor(input: HTMLInputElement, backend: CommandBackend) {
    this.#input = input;
    this.#backend = backend;
  }

  recall(direction: -1 | 1): boolean {
    const history = this.#backend.history;
    if (history.length === 0 || (direction > 0 && this.#index < 0)) {
      return false;
    }

    if (this.#index < 0) {
      this.#draft = this.#input.value;
      this.#index = history.length - 1;
    } else {
      this.#index += direction;

      if (this.#index < 0) {
        this.#index = 0;
      }

      if (this.#index >= history.length) {
        this.#index = -1;
        this.#input.value = this.#draft;
        return true;
      }
    }

    const recalled = history[this.#index];
    if (recalled === undefined) {
      return false;
    }

    this.#input.value = recalled;
    this.#input.setSelectionRange(this.#input.value.length, this.#input.value.length);
    return true;
  }

  /** The user edited the input directly: stop recalling and re-capture the draft. */
  noteEdit(): void {
    if (this.#index >= 0) {
      this.#index = -1;
      this.#draft = this.#input.value;
    }
  }

  /** A command was just submitted: return to the neutral, non-recalling state. */
  reset(): void {
    this.#index = -1;
    this.#draft = "";
  }
}
