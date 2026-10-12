import { test, expect } from "@playwright/test";
import { screenshotColors } from "./pixels.mjs";

test("one browser scalar grid renders both its boundary and optical presentation", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const state = await page.evaluate(async () => {
    const wasm = await import("/dist/molgfx_wasm.js");
    await wasm.default({ module_or_path: "/dist/molgfx_wasm_bg.wasm" });
    const canvas = document.createElement("canvas");
    canvas.width = 640;
    canvas.height = 480;
    document.querySelector("#viewer").append(canvas);
    const scene = wasm.Scene.empty();
    const volume = {
      source: { content_hash: "ball", uri: null, format: null },
      dimensions: [8, 8, 8],
      voxel_to_world: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -4, -4, -4, 1],
      presentations: [
        { kind: "isosurface", isovalue: 0.5, color: [220, 40, 70, 255], opacity: 1, style: { kind: "solid" } },
        { kind: "direct", transfer: [
          { value: 0, color: [50, 150, 255, 255], opacity: 0 },
          { value: 1, color: [50, 150, 255, 255], opacity: 0.4 },
        ], opacity_scale: 1, step_scale: 0.8 },
      ],
      region: null,
    };
    const patch = new wasm.ScenePatch(JSON.stringify({
      base_revision: 0, operations: [{ op: "add_volume", id: 1, volume }],
    }));
    scene.apply(patch);
    patch.free();
    const voxels = new Float32Array(512);
    for (let z = 0; z < 8; z++) for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
      voxels[z * 64 + y * 8 + x] = Math.max(0, 1 - Math.hypot(x - 3.5, y - 3.5, z - 3.5) / 4);
    }
    scene.bindVolume(1n, voxels);
    scene.resolve();
    const renderer = await wasm.Renderer.create(canvas, "interactive");
    const presented = renderer.renderCamera(scene,
      new Float32Array([0, 0, 35]), new Float32Array([0, 0, 0]), new Float32Array([0, 1, 0]));
    window.volumeProof = { renderer, scene };
    return { presented, unresolved: JSON.parse(scene.unresolvedOverlays()) };
  });
  const colors = await screenshotColors(page);
  await page.evaluate(() => { window.volumeProof.renderer.free(); window.volumeProof.scene.free(); });
  expect(state).toEqual({ presented: true, unresolved: [] });
  expect(colors.red).toBeGreaterThan(100);
  expect(colors.blue).toBeGreaterThan(100);
  expect(errors).toEqual([]);
});
test("indexed scalar mesh and dots retain distinct voxel-lattice coverage", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const coverage = [];
  for (const style of [
    { kind: "mesh", line_width_voxels: 0.12 },
    { kind: "dots", dot_radius_voxels: 0.12 },
  ]) {
    const presented = await page.evaluate(async style => {
      const wasm = await import("/dist/molgfx_wasm.js");
      await wasm.default({ module_or_path: "/dist/molgfx_wasm_bg.wasm" });
      const canvas = document.createElement("canvas");
      canvas.width = 256;
      canvas.height = 256;
      document.querySelector("#viewer").append(canvas);
      const scene = wasm.Scene.empty();
      const volume = {
        source: { content_hash: "plane", uri: null, format: null },
        dimensions: [5, 5, 5],
        voxel_to_world: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -2, -2, -2, 1],
        presentations: [{ kind: "isosurface", isovalue: 2,
          color: [30, 100, 230, 255], opacity: 1, style }],
        region: null,
      };
      const patch = new wasm.ScenePatch(JSON.stringify({
        base_revision: 0, operations: [{ op: "add_volume", id: 1, volume }],
      }));
      scene.apply(patch);
      patch.free();
      const values = new Float32Array(125);
      for (let z = 0; z < 5; z++) values.fill(z, z * 25, (z + 1) * 25);
      scene.bindVolume(1n, values);
      scene.resolve();
      const renderer = await wasm.Renderer.create(canvas, "interactive");
      window.latticeProof = { scene, renderer, canvas };
      return renderer.renderCamera(scene, new Float32Array([0, 0, 12]),
        new Float32Array([0, 0, 0]), new Float32Array([0, 1, 0]));
    }, style);
    expect(presented).toBe(true);
    coverage.push((await screenshotColors(page)).blue);
    await page.evaluate(() => {
      window.latticeProof.renderer.free();
      window.latticeProof.scene.free();
      window.latticeProof.canvas.remove();
    });
  }
  expect(coverage[1]).toBeGreaterThan(100);
  expect(coverage[0]).toBeGreaterThan(coverage[1] * 2);
  expect(errors).toEqual([]);
});

test("overlapping gradient streamlines repeat exactly and respond to a replaced potential", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  await page.evaluate(async () => {
    const wasm = await import("/dist/molgfx_wasm.js");
    await wasm.default({ module_or_path: "/dist/molgfx_wasm_bg.wasm" });
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 256;
    document.querySelector("#viewer").append(canvas);
    const scene = wasm.Scene.empty();
    const patch = new wasm.ScenePatch(JSON.stringify({ base_revision: 0, operations: [{
      op: "add_volume", id: 1, volume: {
        source: { content_hash: "gradient", uri: null, format: null }, dimensions: [5, 5, 5],
        voxel_to_world: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -2, -2, -2, 1], region: null,
        presentations: [{ kind: "field_lines", options: {
          seed_stride: 1, seed_gradient: [0, 100], step: 0.125, length: [0.25, 4],
          critical_gradient: 0.000001, max_steps: 64, max_segments: 16384,
          width_pixels: 3, color: [0, 120, 255, 255], opacity: 1,
        } }],
      },
    }] }));
    const volume = JSON.parse(patch.toJSON()).operations[0].volume;
    scene.apply(patch); patch.free();
    const values = Float32Array.from({ length: 125 }, (_, row) => row % 5);
    scene.bindVolume(1n, values); scene.resolve();
    const renderer = await wasm.Renderer.create(canvas, "highest_fixed", 60,
      JSON.stringify([{ kind: "anti_aliasing", settings: "off" }]));
    window.gradientProof = { scene, renderer, volume, render: () => window.gradientProof.renderer.renderCamera(scene,
      new Float32Array([0, 0, 14]), new Float32Array([0, 0, 0]), new Float32Array([0, 1, 0])) };
    window.gradientProof.render();
  });
  const first = await page.locator("#viewer canvas").screenshot();
  expect((await screenshotColors(page)).blue).toBeGreaterThan(100);
  for (let repeat = 0; repeat < 3; repeat++) {
    await page.evaluate(async () => {
      const proof = window.gradientProof;
      const wasm = await import("/dist/molgfx_wasm.js");
      // Compare the first sample of identical fresh exposures; subsequent
      // samples intentionally jitter while temporal coverage accumulates.
      proof.renderer.free();
      proof.renderer = await wasm.Renderer.create(document.querySelector("#viewer canvas"),
        "highest_fixed", 60, JSON.stringify([{ kind: "anti_aliasing", settings: "off" }]));
      proof.render();
    });
    expect(await page.locator("#viewer canvas").screenshot()).toEqual(first);
  }
  await page.evaluate(async () => {
    const { scene, volume, render } = window.gradientProof;
    const wasm = await import("/dist/molgfx_wasm.js");
    const replacement = structuredClone(volume);
    replacement.source.content_hash = "gradient-y";
    const patch = new wasm.ScenePatch(JSON.stringify({ base_revision: Number(scene.revision),
      operations: [{ op: "remove_volume", id: 1 }, { op: "add_volume", id: 1, volume: replacement }] }));
    scene.apply(patch); patch.free();
    scene.bindVolume(1n, Float32Array.from({ length: 125 }, (_, row) => Math.floor(row / 5) % 5));
    scene.resolve(); render();
  });
  expect(await page.locator("#viewer canvas").screenshot()).not.toEqual(first);
  expect((await screenshotColors(page)).blue).toBeGreaterThan(100);
  await page.evaluate(() => { window.gradientProof.renderer.free(); window.gradientProof.scene.free(); });
  expect(errors).toEqual([]);
});

test("one signed amplitude grid retains both orbital phases", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const state = await page.evaluate(async () => {
    const wasm = await import("/dist/molgfx_wasm.js");
    await wasm.default({ module_or_path: "/dist/molgfx_wasm_bg.wasm" });
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 256;
    document.querySelector("#viewer").append(canvas);
    const scene = wasm.Scene.empty();
    const patch = new wasm.ScenePatch(JSON.stringify({ base_revision: 0, operations: [{
      op: "add_volume", id: 1, volume: {
        source: { content_hash: "signed-wavefunction", uri: null, format: null }, dimensions: [9, 9, 9],
        voxel_to_world: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -4, -4, -4, 1], region: null,
        presentations: [{ kind: "orbital", amplitude: 0.25, positive: [235, 45, 30, 255],
          negative: [30, 100, 235, 255], opacity: 1, style: { kind: "solid" } }],
      },
    }] }));
    scene.apply(patch); patch.free();
    const values = new Float32Array(729);
    for (let z = 0; z < 9; z++) for (let y = 0; y < 9; y++) for (let x = 0; x < 9; x++) {
      values[z * 81 + y * 9 + x] = (x - 4) * Math.exp(-((x - 4) ** 2 + (y - 4) ** 2 + (z - 4) ** 2) / 4);
    }
    scene.bindVolume(1n, values); scene.resolve();
    const renderer = await wasm.Renderer.create(canvas, "interactive");
    window.orbitalProof = { scene, renderer };
    return { presented: renderer.renderCamera(scene, new Float32Array([0, 0, 18]),
      new Float32Array([0, 0, 0]), new Float32Array([0, 1, 0])), unresolved: JSON.parse(scene.unresolvedOverlays()) };
  });
  const colors = await screenshotColors(page);
  expect(colors.red).toBeGreaterThan(100);
  expect(colors.blue).toBeGreaterThan(100);
  expect(state).toEqual({ presented: true, unresolved: [] });
  await page.evaluate(() => { window.orbitalProof.renderer.free(); window.orbitalProof.scene.free(); });
  expect(errors).toEqual([]);
});
