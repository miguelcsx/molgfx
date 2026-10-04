import { test, expect } from "@playwright/test";

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

async function screenshotColors(page) {
  const screenshot = await page.locator("#viewer canvas").screenshot();
  return await page.evaluate(async bytes => {
    const bitmap = await createImageBitmap(new Blob([new Uint8Array(bytes)], { type: "image/png" }));
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext("2d");
    context.drawImage(bitmap, 0, 0);
    const pixels = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
    let red = 0, blue = 0;
    for (let i = 0; i < pixels.length; i += 4) {
      const [r, g, b] = pixels.subarray(i, i + 3);
      if (r > b + 35 && r > g + 25) red++;
      if (b > r + 15 && b > g + 3) blue++;
    }
    return { red, blue };
  }, [...screenshot]);
}


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
