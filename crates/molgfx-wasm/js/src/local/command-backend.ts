// The console's backend when commands run in the page: the same grammar and
// session the kernel uses, compiled into the wasm runtime, editing the
// viewer's own scene in place.
import type { CommandBackend, CommandReply, CommandRequest, CompletionItem } from "../core/contracts.js";
import type { Cleanup } from "../core/types.js";
import { patchChangesCamera } from "../runtime/scene.js";
import type { PatchDocument } from "../runtime/scene.js";
import type { ViewerContext } from "../viewer.js";

type Session = InstanceType<ViewerContext["runtime"]["Session"]>;

type Answer =
  | { ok: true; patch?: PatchDocument; revision: number; messages?: string[]; history?: string[] }
  | { ok: false; errors?: { message?: string }[]; rendered?: string };

export class LocalCommandBackend implements CommandBackend {
  readonly #view: ViewerContext;
  readonly #replies = new Set<(reply: CommandReply) => void>();
  readonly #stopWatching: Cleanup;
  #session: Session | undefined;
  #history: string[] = [];

  constructor(view: ViewerContext) {
    this.#view = view;
    this.#stopWatching = view.onSceneReplaced(() => this.#reset());
  }

  get history(): string[] { return this.#history; }

  /** Run command text now and return the reply the console would receive. */
  execute(text: string, id = "local"): CommandReply {
    const scene = this.#view.scene();
    if (!scene) return { id, type: "result", ok: false, text, rendered: "the viewer has no scene" };
    this.#session ??= new this.#view.runtime.Session(scene);
    const answer = JSON.parse(this.#session.execute(scene, text)) as Answer;
    if (!answer.ok) {
      return { id, type: "result", ok: false, text, ...optional("errors", answer.errors), ...optional("rendered", answer.rendered) };
    }
    this.#history = answer.history ?? this.#history;
    this.#view.sceneEdited(patchChangesCamera(answer.patch));
    return { id, type: "result", ok: true, text, revision: answer.revision, ...optional("messages", answer.messages) };
  }

  submit(request: CommandRequest): void {
    let reply: CommandReply;
    try {
      reply = request.type === "execute" ? this.execute(request.text, request.id) : this.#complete(request.id, request.text, request.cursor);
    } catch (error) {
      reply = { id: request.id, type: "result", ok: false, text: request.type === "execute" ? request.text : "", rendered: String(error) };
    }
    // Replies are asynchronous for every backend, so a console never sees one
    // before `submit` returns.
    queueMicrotask(() => { for (const callback of this.#replies) callback(reply); });
  }

  onReply(callback: (reply: CommandReply) => void): Cleanup {
    this.#replies.add(callback);
    return () => { this.#replies.delete(callback); };
  }

  dispose(): void {
    this.#stopWatching();
    this.#replies.clear();
    this.#reset();
  }

  #complete(id: string, text: string, cursor: number): CommandReply {
    const scene = this.#view.scene();
    if (!scene) return { id, type: "completions", items: [] };
    this.#session ??= new this.#view.runtime.Session(scene);
    return { id, type: "completions", items: JSON.parse(this.#session.completions(text, cursor)) as CompletionItem[] };
  }

  /** A new scene starts a new session: names and history belong to the old one. */
  #reset(): void {
    this.#session?.free();
    this.#session = undefined;
    this.#history = [];
  }
}

// `exactOptionalPropertyTypes` forbids assigning `undefined` to an optional key.
function optional<K extends string, V>(key: K, value: V | undefined): { [P in K]?: V } {
  return (value === undefined ? {} : { [key]: value }) as { [P in K]?: V };
}
