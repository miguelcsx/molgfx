import { MAX_COMPLETIONS } from "../core/config.js";
import type { CompletionItem } from "../core/contracts.js";
import { element, setAria } from "../core/dom.js";
import { CleanupBag, listen } from "../core/lifecycle.js";

/** The completion dropdown: its open/closed state, options and keyboard focus. */
export class CompletionController {
  readonly #input: HTMLInputElement;
  readonly #menu: HTMLUListElement;
  readonly #cleanup = new CleanupBag();

  #items: CompletionItem[] = [];
  #active = -1;

  constructor(input: HTMLInputElement, menu: HTMLUListElement) {
    this.#input = input;
    this.#menu = menu;

    this.#cleanup.add(
      listen(menu, "mousedown", (event) => {
        const option = (event.target as HTMLElement).closest<HTMLElement>("li[data-index]");
        if (!option || !menu.contains(option)) {
          return;
        }

        event.preventDefault();

        const index = Number(option.dataset.index);
        const item = Number.isInteger(index) ? this.#items[index] : undefined;
        if (item) {
          this.#replaceWord(item.text);
          this.dismiss();
        }
      }),
    );
  }

  get isOpen(): boolean {
    return this.#items.length > 0;
  }

  /** Show completions, auto-inserting the only candidate rather than opening a one-item menu. */
  show(items: CompletionItem[]): void {
    this.clear();

    if (items.length === 1) {
      this.#replaceWord(items[0]!.text);
      return;
    }

    this.#items = items.slice(0, MAX_COMPLETIONS);
    if (this.#items.length === 0) {
      return;
    }

    const fragment = document.createDocumentFragment();
    this.#items.forEach((item, index) => {
      const option = element("li", "molgfx-completion", item.text);
      option.id = `${this.#menu.id}-option-${index}`;
      option.dataset.index = String(index);
      option.title = item.detail || item.kind || "";
      option.setAttribute("role", "option");
      option.setAttribute("aria-selected", "false");
      fragment.append(option);
    });

    this.#menu.replaceChildren(fragment);
    setAria(this.#input, { expanded: "true" });
  }

  clear(): void {
    this.#items = [];
    this.#active = -1;
    this.#menu.replaceChildren();
    setAria(this.#input, { expanded: "false" });
    this.#input.removeAttribute("aria-activedescendant");
  }

  dismiss(): void {
    this.clear();
  }

  move(delta: number): boolean {
    if (this.#items.length === 0) {
      return false;
    }

    this.#active =
      this.#active < 0
        ? (delta > 0 ? 0 : this.#items.length - 1)
        : (this.#active + delta + this.#items.length) % this.#items.length;

    this.#renderActive();
    return true;
  }

  choose(fallbackToFirst = false): boolean {
    if (this.#items.length === 0) {
      return false;
    }

    const index = this.#active >= 0 ? this.#active : fallbackToFirst ? 0 : -1;
    const selected = index >= 0 ? this.#items[index] : undefined;
    if (!selected) {
      return false;
    }

    this.#replaceWord(selected.text);
    this.dismiss();
    return true;
  }

  dispose(): void {
    this.#cleanup.dispose();
  }

  #replaceWord(text: string): void {
    const input = this.#input;
    const cursor = input.selectionStart ?? input.value.length;
    const before = input.value.slice(0, cursor);
    const start = Math.max(before.lastIndexOf(" "), before.lastIndexOf(","), before.lastIndexOf(";")) + 1;

    input.value = before.slice(0, start) + text + input.value.slice(cursor);

    const caret = start + text.length;
    input.setSelectionRange(caret, caret);
    input.focus();
  }

  #renderActive(): void {
    for (let index = 0; index < this.#menu.children.length; index += 1) {
      const option = this.#menu.children[index] as HTMLElement;
      const active = index === this.#active;
      option.classList.toggle("is-active", active);
      option.setAttribute("aria-selected", String(active));
    }

    if (this.#active < 0) {
      this.#input.removeAttribute("aria-activedescendant");
      return;
    }

    const option = this.#menu.children[this.#active] as HTMLElement;
    this.#input.setAttribute("aria-activedescendant", option.id);
    option.scrollIntoView({ block: "nearest" });
  }
}
