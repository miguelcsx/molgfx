import { test, expect } from "@playwright/test";

const pdb = "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 20.00           C\nATOM      2  CA  GLY A   2       4.000   0.000   0.000  1.00 20.00           C\nEND\n";

test("real WebGPU viewer loads, commands, resizes and disposes repeatedly", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const result = await page.evaluate(async source => {
    const { Viewer } = await import("/dist/index.js");
    const root = document.querySelector("#viewer");
    const loads = [];
    for (let i = 0; i < 2; i++) {
      const start = performance.now();
      const viewer = await Viewer.create(root);
      await viewer.load(new TextEncoder().encode(source), { name: "fixture.pdb" });
      loads.push(performance.now() - start);
      viewer.execute("show spacefill, all");
      viewer.color("orange", "all");
      viewer.select("all");
      viewer.focus("all");
      await new Promise(resolve => setTimeout(resolve, 750));
      const canvas = root.querySelector("canvas");
      if (!canvas || canvas.width < 600 || canvas.height < 400) throw new Error("Canvas failed to acquire its physical extent");
      root.style.width = "320px";
      const deadline = performance.now() + 30000;
      const width = Math.round(320 * devicePixelRatio);
      const height = Math.round(480 * devicePixelRatio);
      while ((canvas.width !== width || canvas.height !== height) && performance.now() < deadline)
        await new Promise(resolve => setTimeout(resolve, 10));
      if (canvas.width !== width || canvas.height !== height) throw new Error("Resize failed: " + JSON.stringify({physical:[canvas.width,canvas.height],css:[canvas.clientWidth,canvas.clientHeight],dpr:devicePixelRatio,error:root.textContent}));
      await viewer.dispose();
      if (root.querySelector("canvas")) throw new Error("Dispose left a canvas mounted");
      root.style.width = "640px";
    }
    return { load_ms: loads };
  }, pdb);
  console.log("viewer lifecycle timings", JSON.stringify(result));
  expect(result.load_ms).toHaveLength(2);
  expect(errors).toEqual([]);
});

test("multiple real viewers isolate state and dispose during an actual GPU pick", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const evidence = await page.evaluate(async source => {
    const { Viewer } = await import("/dist/index.js");
    const runtime = await import("/dist/molgfx_wasm.js");
    let submits = 0;
    const submit = GPUQueue.prototype.submit;
    GPUQueue.prototype.submit = function (...args) { submits++; return submit.apply(this, args); };
    const root = document.querySelector("#viewer");
    const other = document.createElement("div");
    other.style.cssText = "width:640px;height:480px";
    document.body.append(other);
    const a = await Viewer.create(root, { structure: new TextEncoder().encode(source), name: "a.pdb" });
    const b = await Viewer.create(other, { structure: new TextEncoder().encode(source), name: "b.pdb" });
    a.execute("show spacefill, all"); b.execute("show spacefill, all");
    a.focus("all"); b.focus("all");
    const first = a.execute("color orange, all");
    const second = b.execute("color blue, all");
    if (first.revision !== second.revision) throw new Error("Sessions do not begin at independent revisions");
    a.execute("select local, all");
    let rejected = false;
    try { b.execute("color green, $local"); } catch { rejected = true; }
    if (!rejected) throw new Error("Named selection leaked across viewers");
    await new Promise(resolve => setTimeout(resolve, 750));
    let semanticPick;
    let pickPoint;
    const pickStart = performance.now();
    for (const x of [200, 240, 280, 320, 360, 400, 440]) {
      const hit = await a.pick(x, 240);
      if (hit?.pick === "atom") { semanticPick = hit; pickPoint = [x, 240]; break; }
    }
    if (!semanticPick || ![0, 1].includes(semanticPick.atom_index) || !Number.isInteger(semanticPick.dataset) || !Number.isInteger(semanticPick.topology_revision) || semanticPick.structure === undefined) throw new Error("GPU pick lost exact engine provenance: " + JSON.stringify(semanticPick));
    const pickMs = performance.now() - pickStart;
    const canvasForInput = root.querySelector("canvas");
    const cameras = [];
    a.on("camera", camera => cameras.push(camera));
    canvasForInput.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true, cancelable: true }));
    canvasForInput.dispatchEvent(new WheelEvent("wheel", { deltaY: 120, bubbles: true, cancelable: true }));
    await new Promise(resolve => setTimeout(resolve, 300));
    if (cameras.length < 2 || JSON.stringify(cameras[0]) === JSON.stringify(cameras.at(-1))) throw new Error("Real camera interaction did not publish updated engine camera");
    if (!pickPoint) throw new Error("No pick point");
    const idleStart = submits;
    await new Promise(resolve => setTimeout(resolve, 300));
    const idleSubmits = submits - idleStart;
    if (idleSubmits !== 0) throw new Error("Idle viewers submit GPU work");
    let disposed;
    let pickStarted = false;
    let callbacks = 0;
    a.on("pick", () => callbacks++);
    const begin = runtime.Renderer.prototype.beginPick;
    runtime.Renderer.prototype.beginPick = function (...args) {
      const handle = begin.apply(this, args);
      pickStarted = true;
      queueMicrotask(() => { disposed = a.dispose(); });
      return handle;
    };
    const canvas = root.querySelector("canvas");
    const box = canvas.getBoundingClientRect();
    canvas.dispatchEvent(new MouseEvent("click", { bubbles: true, button: 0, clientX: box.x + 320, clientY: box.y + 240 }));
    for (let i = 0; i < 100 && !pickStarted; i++) await new Promise(resolve => setTimeout(resolve, 10));
    if (!pickStarted) throw new Error("Pointer input did not begin a real pick");
    await disposed;
    await b.dispose();
    const afterDispose = submits;
    await new Promise(resolve => setTimeout(resolve, 300));
    runtime.Renderer.prototype.beginPick = begin;
    GPUQueue.prototype.submit = submit;
    if (callbacks || submits !== afterDispose || document.querySelector("canvas")) throw new Error("Work escaped disposal");
    return { semantic_pick: semanticPick, pick_ms: pickMs, camera_events: cameras.length, idle_submits_300ms: idleSubmits, postdispose_submits_300ms: submits - afterDispose, postdispose_pick_callbacks: callbacks, real_pick_started: pickStarted };
  }, pdb);
  console.log("viewer work measurements", JSON.stringify(evidence));
  expect(evidence.real_pick_started).toBe(true);
  expect(errors).toEqual([]);
});

test("unavailable WebGPU rejects creation with actionable capability error and cleans up", async ({ page }) => {
  await page.goto("/tests/browser/host.html");
  const result = await page.evaluate(async () => {
    // Model a real unsupported environment at the browser capability boundary,
    // without replacing the SDK renderer or its behavior.
    let owner = navigator;
    while (owner && !Object.hasOwn(owner, "gpu")) owner = Object.getPrototypeOf(owner);
    if (owner) delete owner.gpu;
    const { Viewer } = await import("/dist/index.js");
    let message;
    try { await Viewer.create(document.querySelector("#viewer")); }
    catch (error) { message = error.message; }
    return { message, canvases: document.querySelectorAll("canvas").length };
  });
  expect(result.message).toContain("WebGPU");
  expect(result.message).toContain("HTTPS or localhost");
  expect(result.canvases).toBe(0);
});
