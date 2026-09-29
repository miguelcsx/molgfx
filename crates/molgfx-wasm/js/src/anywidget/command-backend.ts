import type { CommandBackend, CommandReply, CommandRequest } from "../core/contracts.js";
import type { Cleanup } from "../core/types.js";
import { observeModel } from "./model.js";
import type { WidgetModel } from "./model.js";

/** Adapts one anywidget `WidgetModel` to the console's command backend. */
export class AnywidgetCommandBackend implements CommandBackend {
  readonly #model: WidgetModel;

  constructor(model: WidgetModel) {
    this.#model = model;
  }

  get history(): string[] {
    return this.#model.get("history") || [];
  }

  submit(request: CommandRequest): void {
    this.#model.set("command_request", request);
    this.#model.save_changes();
  }

  onReply(callback: (reply: CommandReply) => void): Cleanup {
    return observeModel(this.#model, "change:command_reply", () => {
      const answer = this.#model.get("command_reply") as CommandReply;
      if ("id" in answer) {
        callback(answer);
      }
    });
  }
}
