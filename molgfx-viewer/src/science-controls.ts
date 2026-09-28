import type { CommandBackend, CommandRequest } from "./core/contracts.js";
import { element, setAria } from "./core/dom.js";
import { CleanupBag, listen } from "./core/lifecycle.js";
import type { Cleanup } from "./core/types.js";

/** A command-backed action exposed by the science control strip. */
export interface ScienceControl {
  readonly id: string;
  readonly label: string;
  readonly command: string;
  readonly detail?: string;
}

/** Options for the reusable, host-independent science controls. */
export interface ScienceControlsOptions {
  /** Selection expression used by target-sensitive actions. */
  readonly target?: string;
  /** Replace the built-in actions. Commands are still parsed by the backend. */
  readonly controls?: readonly ScienceControl[];
  readonly title?: string;
}

const DEFAULT_CONTROLS: readonly ScienceControl[] = [
  { id: "cartoon", label: "Cartoon", command: "show cartoon", detail: "Show the cartoon representation" },
  { id: "ball-and-stick", label: "Ball and stick", command: "show ball_and_stick", detail: "Show atoms and bonds" },
  { id: "surface", label: "Surface", command: "show surface", detail: "Show the molecular surface" },
  { id: "focus", label: "Focus selection", command: "focus $sel", detail: "Fit the camera to the current selection" },
  { id: "unfocus", label: "Clear focus", command: "unfocus", detail: "Restore the unfocused view" },
  { id: "undo", label: "Undo", command: "undo", detail: "Undo the last scene edit" },
  { id: "redo", label: "Redo", command: "redo", detail: "Redo the last scene edit" },
];

let requestSequence = 0;
function request(command: string): CommandRequest {
  requestSequence += 1;
  return { id: `science-${requestSequence}`, type: "execute", text: command };
}

/**
 * Mount a command-backed control strip. Actions use the same parser,
 * transaction, history, and error path as the command console.
 */
export function mountScienceControls(
  backend: CommandBackend,
  parent: HTMLElement,
  options: ScienceControlsOptions = {},
): Cleanup {
  const controls = options.controls ?? DEFAULT_CONTROLS;
  const panel = element("section", "molgfx-science-controls");
  const title = options.title ?? "Science controls";
  panel.setAttribute("aria-label", title);
  panel.appendChild(element("h2", "molgfx-science-controls-title", title));

  const buttons = element("div", "molgfx-science-controls-actions");
  buttons.setAttribute("role", "toolbar");
  buttons.setAttribute("aria-label", title);
  panel.appendChild(buttons);

  const pending = new Map<string, HTMLButtonElement>();
  const cleanup = new CleanupBag();
  for (const control of controls) {
    const button = element("button", "molgfx-science-control", control.label);
    button.type = "button";
    button.id = `molgfx-science-control-${control.id}`;
    if (control.detail) button.title = control.detail;
    setAria(button, { label: control.detail ?? control.label });
    cleanup.add(listen(button, "click", () => {
      const command = control.command.replaceAll("$sel", options.target ?? "$sel");
      const submitted = request(command);
      button.disabled = true;
      pending.set(submitted.id, button);
      backend.submit(submitted);
    }));
    buttons.appendChild(button);
  }

  // Match by request id so out-of-order replies cannot re-enable the wrong control.
  cleanup.add(backend.onReply((reply) => {
    if (reply.type !== "result") return;
    const button = pending.get(reply.id);
    if (!button) return;
    button.disabled = false;
    pending.delete(reply.id);
  }));

  parent.appendChild(panel);
  return () => {
    cleanup.dispose();
    pending.clear();
    panel.remove();
  };
}

export { DEFAULT_CONTROLS };
