import { test, expect } from "@playwright/test";

const pdb = "ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 20.00           C\nEND\n";

test("WASM profiles render authored backdrops and refuse invalid effects", async ({ page }) => {
  await page.goto("/tests/browser/host.html");
  const state = await page.evaluate(async source => {
    const wasm = await import("/dist/molgfx_wasm.js");
    await wasm.default({ module_or_path: "/dist/molgfx_wasm_bg.wasm" });
    const canvas = document.createElement("canvas");
    canvas.width = 128; canvas.height = 128;
    document.querySelector("#viewer").append(canvas);
    const refused = [];
    for (const effects of [
      [{ kind: "bloom", settings: { threshold: 1, intensity: -1, radius: 4 } }],
      [{ kind: "unknown", settings: {} }],
    ]) {
      try { await wasm.Renderer.create(canvas, "interactive", 60, JSON.stringify(effects)); }
      catch (error) { refused.push(String(error)); }
    }
    const scene = wasm.Scene.fromStructureBytes(new TextEncoder().encode(source), "fixture.pdb");
    const session = new wasm.Session(scene);
    const answer = JSON.parse(session.execute(scene, "show spacefill, all"));
    if (!answer.ok) throw new Error(JSON.stringify(answer));
    const red = { r: 220, g: 20, b: 30, a: 255 };
    const renderer = await wasm.Renderer.create(canvas, "interactive", 60, JSON.stringify([
      { kind: "backdrop", settings: { top: red, bottom: red, glow_color: red, glow_strength: 0 } },
      { kind: "anti_aliasing", settings: "fxaa" },
    ]));
    const presented = renderer.renderCamera(scene,
      new Float32Array([0, 0, 15]), new Float32Array([0, 0, 0]), new Float32Array([0, 1, 0]));
    window.effectsProof = { renderer, scene, session };
    return { presented, refused };
  }, pdb);
  const screenshot = await page.locator("#viewer canvas").screenshot();
  const corner = await page.evaluate(async bytes => {
    const bitmap = await createImageBitmap(new Blob([new Uint8Array(bytes)], { type: "image/png" }));
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext("2d"); context.drawImage(bitmap, 0, 0);
    return [...context.getImageData(0, 0, 1, 1).data];
  }, [...screenshot]);
  await page.evaluate(() => {
    window.effectsProof.renderer.free(); window.effectsProof.session.free(); window.effectsProof.scene.free();
  });
  expect(state.presented).toBe(true);
  expect(state.refused).toHaveLength(2);
  expect(corner[0]).toBeGreaterThan(corner[1] + 100);
  expect(corner[0]).toBeGreaterThan(corner[2] + 100);
});
