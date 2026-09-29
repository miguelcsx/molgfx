// The plain-page entry point: `mount(el, options)` embeds the same viewer the
// notebook widget uses, with commands running in the page and no kernel.
import { mountConsole } from "../console/console.js";
import { element, setAria } from "../core/dom.js";
import { errorMessage } from "../core/errors.js";
import type { Cleanup } from "../core/types.js";
import { LocalCommandBackend } from "../local/command-backend.js";
import { LocalHost } from "../local/host.js";
import { mountViewer } from "../viewer.js";

export interface StructureFile {
  /** A file name; its extension picks the parser (`.cif`, `.bcif`, `.pdb`). */
  name: string;
  /** Where to fetch the structure from, in the visitor's browser. */
  url?: string;
  /** The structure itself, when it is already in the page. */
  data?: string | Uint8Array;
}

export interface MountOptions {
  /** The structure shown first. */
  structure: StructureFile;
  /** Shown instead when `structure` cannot be fetched, e.g. offline. */
  fallback?: StructureFile;
  /** Command text run after every structure loads. */
  program?: string;
  /** Offer a file picker for the visitor's own structures. Defaults to true. */
  openFiles?: boolean;
  /** Milliseconds to wait for `structure.url`. Defaults to 15 s. */
  timeoutMs?: number;
}

async function read(file: StructureFile, timeoutMs: number): Promise<Uint8Array> {
  if (typeof file.data === "string") return new TextEncoder().encode(file.data);
  if (file.data) return file.data;
  if (!file.url) throw new Error(`${file.name} has neither data nor a url`);
  const response = await fetch(file.url, { signal: AbortSignal.timeout(timeoutMs) });
  if (!response.ok) throw new Error(`${file.url} answered ${response.status}`);
  return new Uint8Array(await response.arrayBuffer());
}

function createToolbar(openFiles: boolean) {
  const toolbar = element("div", "molgfx-toolbar");
  const status = element("output", "molgfx-toolbar-status", "Loading…");
  setAria(status, { live: "polite" });
  const file = element("input", "molgfx-file-input");
  file.type = "file";
  file.accept = ".cif,.mmcif,.bcif,.pdb,.ent";
  setAria(file, { label: "Open a molecular structure file" });
  if (openFiles) {
    const label = element("label", "molgfx-file");
    label.append(element("span", "molgfx-file-label", "Open structure"), element("span", "molgfx-file-hint", "PDB · mmCIF · BCIF"), file);
    toolbar.append(label);
  }
  toolbar.append(status);
  return { toolbar, status, file };
}

/** Mount a viewer with a command console into `root`; returns its cleanup. */
export function mount(root: HTMLElement, options: MountOptions): Cleanup {
  const timeoutMs = options.timeoutMs ?? 15_000;
  root.replaceChildren();
  const shell = element("section", "molgfx-embed");
  const { toolbar, status, file } = createToolbar(options.openFiles ?? true);
  shell.append(toolbar);
  root.append(shell);

  const report = (message: string, isError = false) => {
    status.textContent = message;
    status.classList.toggle("is-error", isError);
  };

  let disposed = false;
  let failed = false;
  let unmountViewer: Cleanup | undefined;
  let backend: LocalCommandBackend | undefined;

  const runProgram = () => {
    if (!options.program || !backend) return;
    const reply = backend.execute(options.program);
    if (reply.type === "result" && !reply.ok) report(reply.rendered ?? "The starting program failed.", true);
  };

  const start = async () => {
    let initial = options.structure;
    let bytes: Uint8Array;
    try {
      report(`Fetching ${initial.name}…`);
      bytes = await read(initial, timeoutMs);
    } catch (error) {
      if (!options.fallback) throw error;
      initial = options.fallback;
      bytes = await read(initial, timeoutMs);
    }
    if (disposed) return;
    const host = new LocalHost(initial.name, bytes, {
      onError: (error) => { failed = true; report(errorMessage(error), true); },
      onPick: (result) => report(result === undefined ? "Background" : result),
    });
    const unmount = await mountViewer({
      el: shell,
      source: host,
      sink: host,
      mountConsole: (parent, view) => {
        const local = new LocalCommandBackend(view);
        backend = local;
        const disposeConsole = mountConsole(local, parent);
        return () => { disposeConsole(); local.dispose(); backend = undefined; };
      },
    });
    if (disposed) { unmount(); return; }
    unmountViewer = unmount;
    runProgram();
    report(`${initial.name} ready.`);

    file.addEventListener("change", async () => {
      const selected = file.files?.[0];
      if (!selected) return;
      file.disabled = true;
      try {
        report(`Loading ${selected.name}…`);
        failed = false;
        // The viewer reports a file it cannot parse and keeps the previous scene.
        host.load(selected.name, new Uint8Array(await selected.arrayBuffer()));
        if (failed) return;
        runProgram();
        report(`${selected.name} ready.`);
      } catch (error) {
        report(errorMessage(error), true);
      } finally {
        file.disabled = false;
      }
    });
  };

  start().catch((error) => report("MolGFX could not start: " + errorMessage(error), true));

  return () => {
    disposed = true;
    unmountViewer?.();
    root.replaceChildren();
  };
}
