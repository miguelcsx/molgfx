import { test, expect } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { screenshotColors } from "./pixels.mjs";

const root = fileURLToPath(new URL("../../../", import.meta.url));

for (const method of ["volume", "hydrogen_bonds", "surf", "msms", "trajectory", "sampled_surf", "sampled_msms", "sampled_hydrogen_bonds", "candidate_hydrogen_bonds", "playback", "playback_hydrogen_bonds", "sampled_playback_hydrogen_bonds"]) {
  test(`canonical AnyWidget restores ${method} assets and applies the next authored revision`, async ({ page }) => {
    const actualMethod = method.replace("sampled_", "");
    const sampled = method.startsWith("sampled_");
    const playback = method.includes("playback");
    const expires = sampled || (playback && method.endsWith("hydrogen_bonds"));
    const moving = sampled || playback || method === "trajectory" || method === "candidate_hydrogen_bonds";
    test.skip((actualMethod === "surf" || actualMethod === "msms") && !process.env[`MOLFRAME_${actualMethod.toUpperCase()}`], "requires caller-installed scientific provider");
    const errors = [];
    page.on("pageerror", error => errors.push(error.message));
    const python = process.env.MOLGFX_PYTHON ?? resolve(root, ".venv/bin/python");
    const states = JSON.parse(execFileSync(python, ["python/tests/headless_scientific_widget_fixture.py", method], { cwd: root, maxBuffer: 64 * 1024 * 1024 }));
    for (const key of Object.keys(states).filter(key => key !== "initial")) {
      delete states[key]._runtime_js; delete states[key]._runtime_wasm; delete states[key]._runtime_key;
    }
    await page.route("**/__scientific_widget", route => route.fulfill({ contentType: "application/json", body: JSON.stringify(states) }));
    await page.goto("/tests/browser/host.html");
    await page.evaluate(async () => {
      const encoded = await (await fetch("/__scientific_widget")).json();
      const bytes = value => new DataView(Uint8Array.from(atob(value), char => char.charCodeAt(0)).buffer);
      const decode = state => ({ ...state,
        ...(state._runtime_wasm ? { _runtime_wasm: bytes(state._runtime_wasm) } : {}),
        structure_payloads: state.structure_payloads.map(bytes),
        coordinate_frames: (state.coordinate_frames ?? []).map(([id, index, seconds, positions]) => [id, index, seconds, bytes(positions)]),
        volume_bindings: state.volume_bindings.map(([id, values]) => [id, bytes(values)]),
        hydrogen_bond_bindings: state.hydrogen_bond_bindings.map(([id, rows, distance, pose]) => [id, bytes(rows), distance, pose]),
        molecular_surface_bindings: state.molecular_surface_bindings.map(([id, metadata, ...columns]) => [id, metadata, ...columns.map(bytes)]),
      });
      // Implements the WidgetModel port; all scientific computation and rendering are real.
      class Model {
        values; listeners = new Map();
        constructor(values) { this.values = values; }
        get(key) { return this.values[key]; }
        set(key, value) { this.batch({ [key]: value }); }
        batch(values) {
          const changed = Object.keys(values).filter(key => this.values[key] !== values[key]);
          Object.assign(this.values, values);
          for (const key of changed) for (const callback of this.listeners.get("change:" + key) ?? []) callback();
        }
        on(event, callback) { const callbacks = this.listeners.get(event) ?? new Set(); callbacks.add(callback); this.listeners.set(event, callbacks); }
        off(event, callback) { this.listeners.get(event)?.delete(callback); }
        save_changes() {}
      }
      const states = Object.fromEntries(Object.entries(encoded).map(([key, value]) => [key, decode(value)]));
      const { default: entry } = await import("/dist/anywidget.js");
      const model = new Model(states.initial);
      window.scientificWidget = { model, states, entry, close: await entry.render({ model, el: document.querySelector("#viewer") }) };
    });
    await expect(page.locator("#viewer canvas")).toBeVisible();
    await expect.poll(async () => {
      expect(errors).toEqual([]);
      expect(await page.evaluate(() => window.scientificWidget.model.get("error"))).toBe("");
      const colors = await screenshotColors(page);
      return moving || actualMethod === "surf" || actualMethod === "msms" || method === "trajectory" ? colors.red : colors.blue;
    }, { timeout: 30000 }).toBeGreaterThan(method === "hydrogen_bonds" ? 20 : 100);
    if (moving) {
      const original = await page.locator("#viewer canvas").screenshot();
      await page.evaluate(playback => {
        const { model, states } = window.scientificWidget;
        model.batch(playback ? {
          coordinate_frames: states.advanced.coordinate_frames,
          playback_bindings: states.advanced.playback_bindings,
          playback_pair: states.advanced.playback_pair,
        } : { trajectory_sample: states.advanced.trajectory_sample });
      }, playback);
      if (expires) {
        await expect.poll(async () => (await screenshotColors(page)).red).toBe(0);
      } else {
        await expect.poll(async () => !(await page.locator("#viewer canvas").screenshot()).equals(original)).toBe(true);
      }
      expect(await page.evaluate(() => ({ error: window.scientificWidget.model.get("error"), sync: window.scientificWidget.model.get("sync_request"), revision: window.scientificWidget.model.get("revision"), patch: window.scientificWidget.model.get("patch_sequence") }))).toEqual({ error: "", sync: 0, revision: states.initial.revision, patch: states.initial.patch_sequence });
      // Reopen from a complete snapshot at a different sample; the old surface stays stale.
      await page.evaluate(async () => {
        const widget = window.scientificWidget;
        widget.close();
        widget.model.batch(widget.states.stale_resync);
        widget.close = await widget.entry.render({ model: widget.model, el: document.querySelector("#viewer") });
      });
      await expect(page.locator("#viewer canvas")).toBeVisible();
      if (expires) await expect.poll(async () => (await screenshotColors(page)).red).toBe(0);
      await page.evaluate(playback => {
        const { model, states } = window.scientificWidget;
        model.batch(playback ? {
          coordinate_frames: states.reversed.coordinate_frames,
          playback_bindings: states.reversed.playback_bindings,
          playback_pair: states.reversed.playback_pair,
        } : { trajectory_sample: states.reversed.trajectory_sample });
      }, playback);
      await expect.poll(async () => (await screenshotColors(page)).red).toBeGreaterThan(100);
      expect(await page.evaluate(() => window.scientificWidget.model.get("error"))).toBe("");
    }
    await page.evaluate(() => {
      const { model, states } = window.scientificWidget;
      model.batch({ scene_patch: states.patched.scene_patch, revision: states.patched.revision, patch_sequence: states.patched.patch_sequence });
    });
    await page.waitForTimeout(300);
    expect(await page.evaluate(() => ({ error: window.scientificWidget.model.get("error"), sync: window.scientificWidget.model.get("sync_request") }))).toEqual({ error: "", sync: moving ? states.stale_resync.sync_request : 0 });
    await page.evaluate(() => window.scientificWidget.model.batch(window.scientificWidget.states.resynced));
    await page.waitForTimeout(300);
    expect(await page.evaluate(() => window.scientificWidget.model.get("error"))).toBe("");
    await page.evaluate(() => window.scientificWidget.close());
    expect(errors).toEqual([]);
  });
}
