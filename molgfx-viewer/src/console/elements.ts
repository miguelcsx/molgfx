import { element, setAria } from "../core/dom.js";

let consoleSequence = 0;

export interface ConsoleElements {
  id: number;
  panel: HTMLDivElement;
  log: HTMLOListElement;
  input: HTMLInputElement;
  menu: HTMLUListElement;
  status: HTMLPreElement;
}

export function createConsoleElements(): ConsoleElements {
  const id = ++consoleSequence;

  const panel = element("div", "molgfx-console");
  const log = element("ol", "molgfx-log");
  const row = element("div", "molgfx-prompt");
  const prompt = element("span", "molgfx-caret", "›");
  const input = element("input", "molgfx-input");
  const menu = element("ul", "molgfx-completions");
  const status = element("pre", "molgfx-status");

  input.type = "text";
  input.spellcheck = false;
  input.autocomplete = "off";
  input.placeholder = "show cartoon, protein   (Tab completes, ↑↓ history)";
  input.setAttribute("role", "combobox");
  setAria(input, {
    label: "MolGFX command",
    autocomplete: "list",
    expanded: "false",
    controls: `molgfx-completions-${id}`,
  });

  prompt.setAttribute("aria-hidden", "true");

  menu.id = `molgfx-completions-${id}`;
  menu.setAttribute("role", "listbox");

  setAria(log, { label: "MolGFX command history" });

  status.setAttribute("role", "status");
  setAria(status, { live: "polite" });

  row.append(prompt, input);
  panel.append(log, row, menu, status);

  return { id, panel, log, input, menu, status };
}

export function formatErrors(errors: Array<{ message?: string }>): string {
  return errors.map((error) => error.message ?? String(error)).join("\n");
}
