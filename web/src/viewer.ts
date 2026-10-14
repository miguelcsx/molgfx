import { ViewerCore } from "./runtime/core.js";
import type { Transport } from "./runtime/transport.js";
import type { SequenceManifest, SequenceOptions, TransportSetup } from "./core/contracts.js";
import type {
  CommandResult,
  LoadOptions,
  Pick,
  ShowOptions,
  StructureInput,
  ViewerEvents,
  ViewerOptions,
} from "./types.js";
export type {
  CameraState,
  CommandResult,
  LoadOptions,
  Pick,
  PickEvent,
  SelectionChangeEvent,
  SelectionModifiers,
  ShowOptions,
  StructureBytes,
  StructureInput,
  ViewerEvents,
  ViewerOptions,
} from "./types.js";
/** A reusable canvas host; all authoring commands are evaluated by the Rust Session. */
export class Viewer {
  readonly #core: ViewerCore;
  private constructor(core: ViewerCore) {
    this.#core = core;
  }
  static async create(
    element: HTMLElement,
    options: ViewerOptions = {},
  ): Promise<Viewer> {
    const core = new ViewerCore(element, options);
    await core.initialize(options);
    return new Viewer(core);
  }
  load(input: StructureInput, options: LoadOptions = {}): Promise<void> {
    return this.#core.load(input, options);
  }
  execute(text: string): CommandResult {
    return this.#core.execute(text);
  }
  show(form: string, target = "all", options: ShowOptions = {}): CommandResult {
    return this.execute(
      "show " +
        form +
        (options.name === undefined ? "" : " as " + options.name) +
        ", " +
        target,
    );
  }
  hide(layer: string): CommandResult {
    return this.execute(
      "hide " + (layer.startsWith("@") ? layer : "@" + layer),
    );
  }
  color(value: string, target = "all"): CommandResult {
    return this.execute("color " + value + ", " + target);
  }
  focus(target = "all"): CommandResult {
    return this.execute("focus " + target);
  }
  select(query?: string): CommandResult {
    return this.#core.select(query);
  }
  pick(x: number, y: number): Promise<Pick | undefined> {
    return this.#core.pick(x, y);
  }
  /** Headless playback over fitted tracks; the viewer draws continuously only while it plays. */
  attachTransport(setup: TransportSetup): Transport {
    return this.#core.attachTransport(setup);
  }
  /** Captures a deterministic frame sequence over the attached transport and returns its manifest. */
  captureSequence(options: SequenceOptions): Promise<SequenceManifest> {
    return this.#core.captureSequence(options);
  }
  on<K extends keyof ViewerEvents>(
    type: K,
    callback: (event: ViewerEvents[K]) => void,
  ): () => void {
    return this.#core.events.on(type, callback);
  }
  /** Immediately stops all input/scheduling; resolves after detached readbacks and wasm frees. */
  dispose(): Promise<void> {
    return this.#core.dispose();
  }
}
