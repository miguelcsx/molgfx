import type { CommandBackend, CommandReply, CommandRequest } from "../core/contracts.js";
import { element } from "../core/dom.js";
import { CleanupBag, listen } from "../core/lifecycle.js";
import type { Cleanup } from "../core/types.js";
import { CompletionController } from "./completion.js";
import { createConsoleElements, formatErrors } from "./elements.js";
import { HistoryNavigator } from "./history.js";

type RequestPayload =
  | { type: "execute"; text: string }
  | { type: "complete"; text: string; cursor: number };

export function mountConsole(backend: CommandBackend, parent: HTMLElement): Cleanup {
  const { id: consoleId, panel, log, input, menu, status } = createConsoleElements();
  parent.appendChild(panel);

  const cleanup = new CleanupBag();
  const completion = new CompletionController(input, menu);
  const history = new HistoryNavigator(input, backend);
  cleanup.add(() => completion.dispose());

  const pending = new Set<string>();
  let requestSequence = 0;
  let latestCompletionId: string | undefined;
  let latestExecutionId: string | undefined;

  const request = (payload: RequestPayload): string => {
    const id = `${consoleId}-${Date.now()}-${++requestSequence}`;
    pending.add(id);

    const command: CommandRequest = { ...payload, id };
    backend.submit(command);

    return id;
  };

  const appendEntry = (text: string, ok: boolean, detail?: string) => {
    const item = element("li", ok ? "molgfx-ok" : "molgfx-failed");
    item.append(element("code", undefined, text));

    if (detail) {
      item.append(element("div", "molgfx-detail", detail));
    }

    log.append(item);
    log.scrollTop = log.scrollHeight;
  };

  const handleReply = (answer: CommandReply) => {
    if (!pending.delete(answer.id)) {
      return;
    }

    if (answer.type === "completions") {
      if (answer.id === latestCompletionId) {
        latestCompletionId = undefined;
        completion.show(answer.items || []);
      }
      return;
    }

    if (answer.ok) {
      appendEntry(answer.text, true, (answer.messages || []).join("\n"));

      if (answer.id === latestExecutionId) {
        latestExecutionId = undefined;
        status.textContent = `revision ${answer.revision}`;
        status.className = "molgfx-status";
      }
    } else {
      appendEntry(answer.text, false, formatErrors(answer.errors || []));

      if (answer.id === latestExecutionId) {
        latestExecutionId = undefined;
        status.textContent = answer.rendered || "";
        status.className = "molgfx-status molgfx-error";
      }
    }
  };

  const execute = (): boolean => {
    if (input.value.trim() === "") {
      return false;
    }

    completion.dismiss();
    latestExecutionId = request({ type: "execute", text: input.value });
    status.textContent = "running…";
    status.className = "molgfx-status";
    input.value = "";
    history.reset();

    return true;
  };

  const complete = (): boolean => {
    if (completion.choose(true)) {
      return true;
    }

    latestCompletionId = request({
      type: "complete",
      text: input.value,
      cursor: input.selectionStart ?? input.value.length,
    });

    return true;
  };

  const KEY_HANDLERS: Record<string, () => boolean> = {
    Escape: () => {
      const wasOpen = completion.isOpen;
      completion.dismiss();
      return wasOpen;
    },
    ArrowDown: () => (completion.isOpen ? completion.move(1) : history.recall(1)),
    ArrowUp: () => (completion.isOpen ? completion.move(-1) : history.recall(-1)),
    Tab: () => complete(),
    Enter: () => completion.choose() || execute(),
  };

  cleanup.add(
    listen(input, "keydown", (event) => {
      event.stopPropagation();
      if (event.isComposing) {
        return;
      }

      const handled = KEY_HANDLERS[event.key]?.() ?? false;
      if (handled) {
        event.preventDefault();
      }
    }),
  );

  cleanup.add(listen(input, "keypress", (event) => event.stopPropagation()));
  cleanup.add(listen(input, "keyup", (event) => event.stopPropagation()));

  cleanup.add(
    listen(input, "input", () => {
      completion.dismiss();
      history.noteEdit();
    }),
  );

  cleanup.add(backend.onReply(handleReply));

  return () => {
    pending.clear();
    cleanup.dispose();
    panel.remove();
  };
}
