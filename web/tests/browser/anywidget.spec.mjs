import { test, expect } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../", import.meta.url));
function fixture() {
  const python = process.env.MOLGFX_PYTHON ?? resolve(root, ".venv/bin/python");
  const state = JSON.parse(execFileSync(python, ["python/tests/headless_widget_fixture.py"], { cwd: root, maxBuffer: 64 * 1024 * 1024 }));
  for (const key of ["patched", "replacement", "resynced"]) {
    delete state[key]._runtime_js; delete state[key]._runtime_wasm; delete state[key]._runtime_key;
  }
  return state;
}
async function prepare(page) {
  const state = fixture();
  await page.route("**/__widget_fixture", route => route.fulfill({ contentType: "application/json", body: JSON.stringify(state) }));
  await page.goto("/tests/browser/host.html");
  await page.evaluate(async () => {
    const serialized = await (await fetch("/__widget_fixture")).json();
    const bytes = value => Uint8Array.from(atob(value), char => char.charCodeAt(0));
    const decode = state => ({ ...state, ...(state._runtime_wasm ? { _runtime_wasm: new DataView(bytes(state._runtime_wasm).buffer) } : {}), structure_payloads: state.structure_payloads.map(value => new DataView(bytes(value).buffer)) });
    const states = Object.fromEntries(Object.entries(serialized).map(([key, value]) => [key, decode(value)]));
    // This is the WidgetModel transport port, not a mock renderer or Session.
    // Like Backbone.set(object), all trait values commit before change events.
    class Model {
      values; listeners = new Map(); saves = 0;
      constructor(values) { this.values = { ...values }; }
      get(key) { return this.values[key]; }
      set(key, value) { this.batch({ [key]: value }); }
      batch(values) {
        const changed = Object.keys(values).filter(key => this.values[key] !== values[key]);
        Object.assign(this.values, values);
        for (const key of changed) for (const callback of this.listeners.get("change:" + key) ?? []) callback();
      }
      on(event, callback) { const set = this.listeners.get(event) ?? new Set(); set.add(callback); this.listeners.set(event, set); }
      off(event, callback) { this.listeners.get(event)?.delete(callback); }
      save_changes() { this.saves++; }
      listenerCount() { return [...this.listeners.values()].reduce((count, set) => count + set.size, 0); }
    }
    const glue = await (await fetch("/dist/anywidget.js")).text();
    const url = URL.createObjectURL(new Blob([glue], { type: "text/javascript" }));
    const entry = (await import(url)).default; URL.revokeObjectURL(url);
    let submits = 0;
    const submit = GPUQueue.prototype.submit;
    GPUQueue.prototype.submit = function (...args) { submits++; window.widget.queue = this; return submit.apply(this, args); };
    window.widget = { states, Model, entry, model: new Model(states.initial), submits: () => submits };
    window.widget.close = await entry.render({ model: window.widget.model, el: document.querySelector("#viewer") });
  });
}

test("real Python AnyWidget state renders, picks, patches, resyncs and closes", async ({ page }) => {
  const errors = []; page.on("pageerror", error => errors.push(error.message));
  await prepare(page);
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible();
  let initial;
  // Observe presentation, not a fixed delay or a pick that forces another draw.
  await expect.poll(async () => {
    expect(errors).toEqual([]);
    expect(await page.evaluate(() => window.widget.model.get("error"))).toBe("");
    initial = await canvas.screenshot();
    return page.evaluate(async encoded => {
      const bytes = Uint8Array.from(atob(encoded), char => char.charCodeAt(0));
      const image = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
      const context = new OffscreenCanvas(image.width, image.height).getContext("2d");
      context.drawImage(image, 0, 0); image.close();
      const pixels = context.getImageData(0, 0, context.canvas.width, context.canvas.height).data;
      let count = 0;
      for (let index = 0; index < pixels.length; index += 4) {
        if (Math.max(pixels[index], pixels[index + 1], pixels[index + 2]) - Math.min(pixels[index], pixels[index + 1], pixels[index + 2]) > 24) count++;
      }
      return count;
    }, initial.toString("base64"));
  }, { timeout: 30000 }).toBeGreaterThan(20);
  // Background-only presentation must not pass just because later picks draw.
  const pick = await page.evaluate(async () => {
    const { model } = window.widget, canvas = document.querySelector("canvas"), bounds = canvas.getBoundingClientRect();
    const spec = model.get("scene_spec");
    for (const [x, y] of [[0.5,0.5],[0.4,0.5],[0.6,0.5],[0.4,0.35],[0.6,0.65]]) {
      const eventId = model.get("interaction_event")?.eventId;
      canvas.dispatchEvent(new MouseEvent("click", { clientX: bounds.left + bounds.width * x, clientY: bounds.top + bounds.height * y, bubbles: true }));
      const deadline = performance.now() + 5000;
      while (model.get("interaction_event")?.eventId === eventId) { if (performance.now() > deadline) throw new Error("Native pick did not return"); await new Promise(resolve => setTimeout(resolve, 10)); }
      if (model.get("pick").pick) {
        if (model.get("scene_spec") !== spec) throw new Error("Pick altered canonical selection");
        return { pick: model.get("pick"), event: model.get("interaction_event") };
      }
    }
    throw new Error("Rendered molecule produced no semantic pick");
  });
  expect(pick.pick.pick).toBeTruthy();
  expect(JSON.parse(pick.event.result)).toEqual(pick.pick);
  expect(pick.pick).toHaveProperty("structure");
  const before = await page.evaluate(() => window.widget.submits());
  await page.evaluate(() => {
    const { model, states } = window.widget;
    model.batch({ scene_patch: states.patched.scene_patch, revision: states.patched.revision, patch_sequence: states.patched.patch_sequence });
  });
  await page.waitForTimeout(700);
  expect(await page.evaluate(() => window.widget.submits())).toBeGreaterThan(before);
  expect((await canvas.screenshot()).equals(initial)).toBe(false);
  await page.evaluate(() => window.widget.model.batch(window.widget.states.replacement));
  await page.waitForTimeout(400);
  const shared = await page.evaluate(() => ({ error: window.widget.model.get("error"), sources: window.widget.model.get("structure_sources"), lengths: window.widget.model.get("structure_payloads").map(value => value.byteLength) }));
  expect(shared.error).toBe(""); expect(shared.sources).toEqual([1, 1]);
  expect(shared.lengths[0]).toBeGreaterThan(0); expect(shared.lengths[1]).toBe(0);
  const sync = await page.evaluate(() => {
    const { model, states } = window.widget;
    const before = model.get("sync_request");
    model.batch({ scene_patch: states.patched.scene_patch, patch_sequence: model.get("patch_sequence") + 1 });
    const after = model.get("sync_request");
    model.batch(states.resynced);
    return { before, after };
  });
  expect(sync.after).toBe(sync.before + 1);
  const closed = await page.evaluate(async () => {
    const { model, states } = window.widget;
    window.widget.close();
    const saves = model.saves, submits = window.widget.submits();
    model.batch(states.initial);
    await new Promise(resolve => setTimeout(resolve, 500));
    return { listeners: model.listenerCount(), canvases: document.querySelectorAll("canvas").length, saves: model.saves - saves, submissions: window.widget.submits() - submits };
  });
  expect(closed).toEqual({ listeners: 0, canvases: 0, saves: 0, submissions: 0 });
  expect(errors).toEqual([]);
});

test("AnyWidget reads latest binary snapshot after inline runtime await", async ({ page }) => {
  const errors = []; page.on("pageerror", error => errors.push(error.message));
  await prepare(page);
  const result = await page.evaluate(async () => {
    const { Model, states, entry } = window.widget;
    window.widget.close();
    const model = new Model({ ...states.initial, _runtime_key: states.initial._runtime_key + "-await-race" });
    const original = WebAssembly.instantiate; let release, entered;
    const gate = new Promise(resolve => { release = resolve; }), started = new Promise(resolve => { entered = resolve; });
    WebAssembly.instantiate = async function (...args) { entered(); await gate; return original.apply(this, args); };
    let close;
    try {
      const pending = entry.render({ model, el: document.querySelector("#viewer") });
      await started; model.batch(states.replacement); release(); close = await pending;
      await new Promise(resolve => setTimeout(resolve, 500));
      const snapshot = { error: model.get("error"), resync: model.get("sync_request"), revision: model.get("revision"), sources: model.get("structure_sources"), canvas: !!document.querySelector("canvas") };
      close(); return { ...snapshot, listeners: model.listenerCount() };
    } finally { WebAssembly.instantiate = original; close?.(); }
  });
  expect(result).toEqual({ error: "", resync: 0, revision: 3, sources: [1, 1], canvas: true, listeners: 0 });
  expect(errors).toEqual([]);
});

test("AnyWidget requests full resync for a patch missed during GPU initialization", async ({ page }) => {
  const errors = []; page.on("pageerror", error => errors.push(error.message));
  await prepare(page);
  const result = await page.evaluate(async () => {
    const { Model, states, entry } = window.widget;
    window.widget.close();
    const model = new Model(states.initial), original = navigator.gpu.requestAdapter;
    let entered, release;
    const started = new Promise(resolve => { entered = resolve; }), gate = new Promise(resolve => { release = resolve; });
    navigator.gpu.requestAdapter = async function (...args) { entered(); await gate; return original.apply(this, args); };
    const save = model.save_changes.bind(model); let resyncs = 0;
    model.save_changes = () => { save(); if (model.get("sync_request") > resyncs) { resyncs++; model.batch(states.replacement); } };
    let close;
    try {
      const pending = entry.render({ model, el: document.querySelector("#viewer") });
      await started;
      model.batch({ scene_patch: states.patched.scene_patch, revision: states.patched.revision, patch_sequence: states.patched.patch_sequence });
      release(); close = await pending;
      await new Promise(resolve => setTimeout(resolve, 500));
      const observed = { resyncs, error: model.get("error"), revision: model.get("revision"), canvas: !!document.querySelector("canvas") };
      close(); return { ...observed, listeners: model.listenerCount() };
    } finally { navigator.gpu.requestAdapter = original; close?.(); }
  });
  expect(result).toEqual({ resyncs: 1, error: "", revision: 3, canvas: true, listeners: 0 });
  expect(errors).toEqual([]);
});

test("large wheel movements retain a finite interactive camera", async ({ page }) => {
  await prepare(page);
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible();
  await canvas.dispatchEvent("wheel", { deltaY: 2400, bubbles: true, cancelable: true });
  await expect.poll(() => page.evaluate(() => {
    const model = window.widget.model;
    if (model.get("error")) return model.get("error");
    const camera = model.get("camera");
    return [camera.position, camera.target, camera.up].every(
      vector => Array.isArray(vector) && vector.length === 3 && vector.every(Number.isFinite),
    );
  })).toBe(true);
  expect(await page.evaluate(() => window.widget.model.get("error"))).toBe("");
  await page.evaluate(() => window.widget.close());
});

test("hovering a settled scene does not restart presentation sampling", async ({ page }) => {
  await prepare(page);
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible();
  await expect.poll(async () => {
    const before = await page.evaluate(async () => {
      await window.widget.queue.onSubmittedWorkDone();
      return window.widget.submits();
    });
    await page.waitForTimeout(200);
    return (await page.evaluate(() => window.widget.submits())) === before;
  }).toBe(true);
  await page.evaluate(() => window.widget.queue.onSubmittedWorkDone());
  const before = await canvas.screenshot();
  await canvas.hover({ position: { x: 100, y: 100 } });
  await page.waitForTimeout(300);
  await canvas.hover({ position: { x: 130, y: 120 } });
  await page.waitForTimeout(300);
  expect((await canvas.screenshot()).equals(before)).toBe(true);
  expect(await page.evaluate(() => window.widget.model.get("error"))).toBe("");
  await page.evaluate(() => window.widget.close());
});
