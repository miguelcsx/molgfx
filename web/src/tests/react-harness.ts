import { createElement, StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { MolGFXViewer } from "../react/index.js";
import type { Viewer } from "../viewer.js";
async function until(check: () => boolean): Promise<void> {
  const deadline = performance.now() + 10000;
  while (!check()) { if (performance.now() > deadline) throw new Error("React lifecycle did not settle"); await new Promise<void>((resolve) => requestAnimationFrame(() => resolve())); }
}
/** Runs against ReactDOM and the real WebGPU SDK, not a second renderer or runtime mock. */
export async function runReactLifecycleTests(bytes: Uint8Array): Promise<string[]> {
  const host = document.createElement("div"); host.style.cssText = "width:320px;height:240px"; document.body.append(host);
  const root = createRoot(host); const ready: Viewer[] = [], errors: string[] = []; let selections = 0;
  const firstAbort = new AbortController();
  const render = (name: string, signal = firstAbort.signal, structure = bytes) => flushSync(() => root.render(createElement(StrictMode, null, createElement(MolGFXViewer, {
    structure, signal, name, style: { height: "240px" }, onReady: (viewer) => ready.push(viewer),
    onError: (error) => errors.push(error.message), onSelectionChange: () => { selections++; },
  }))));
  try {
    render("first.pdb"); await until(() => ready.length === 1 && selections >= 1);
    const canvas = host.querySelector("canvas"), viewer = ready[0]!;
    render("second.pdb"); await until(() => selections >= 2);
    if (host.querySelector("canvas") !== canvas || ready.length !== 1) throw new Error("React update recreated the viewer");
    let obsoleteLoads = 0; const load = viewer.load.bind(viewer);
    viewer.load = (...args) => { obsoleteLoads++; return load(...args); };
    const secondAbort = new AbortController();
    render("signal-update.pdb", secondAbort.signal, bytes.slice());
    await until(() => ready.length === 2 && selections >= 3);
    const next = ready[1]!;
    if (obsoleteLoads || next === viewer || host.querySelector("canvas") === canvas) throw new Error("Signal recreation reused obsolete viewer or loaded into it");
    firstAbort.abort();
    if (!host.querySelector("canvas")) throw new Error("Obsolete signal disposed the current viewer");
    next.select("all"); secondAbort.abort();
    await until(() => host.querySelector("canvas") === null);
    flushSync(() => root.unmount()); await Promise.all([viewer.dispose(), next.dispose()]);
    if (host.querySelector("canvas")) throw new Error("React unmount leaked canvas");
    const after = selections;
    await new Promise<void>((resolve) => setTimeout(resolve, 250));
    if (selections !== after || errors.length) throw new Error("React published after unmount or reported engine error: " + errors.join(", "));
  } finally { host.remove(); }
  const pendingHost = document.createElement("div"); document.body.append(pendingHost); const pendingRoot = createRoot(pendingHost); let late = 0;
  flushSync(() => pendingRoot.render(createElement(MolGFXViewer, { structure: bytes, onReady: () => { late++; }, onError: () => { late++; } })));
  flushSync(() => pendingRoot.unmount());
  await new Promise<void>((resolve) => setTimeout(resolve, 500));
  if (late || pendingHost.querySelector("canvas")) throw new Error("Late viewer creation survived React unmount");
  pendingHost.remove();
  return ["real React StrictMode mount", "structure prop update preserves viewer", "external signal and structure update isolate active generation", "unmount disposes viewer without late events", "unmount during creation produces no late callback or canvas"];
}
