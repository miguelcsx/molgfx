import { test, expect } from "@playwright/test";

test("two styled label grids keep independent GPU pick identities across resolution and removal", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const result = await page.evaluate(async () => {
    const wasm = await import("/dist/molgfx_wasm.js");
    await wasm.default({ module_or_path: "/dist/molgfx_wasm_bg.wasm" });
    const canvas = document.createElement("canvas");
    canvas.width = 640;
    canvas.height = 480;
    document.querySelector("#viewer").append(canvas);
    const scene = wasm.Scene.empty();
    const identities = [];
    for (const [name, x, color] of [
      ["left", -9, [255, 60, 20, 255]],
      ["right", 2, [20, 180, 255, 255]],
    ]) {
      const specification = {
        source: { content_hash: name, uri: null, format: null },
        dimensions: [8, 8, 8],
        voxel_to_world: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, -4, -4, 1],
        styles: [{ label: 7, color, opacity: 1, visible: true }],
      };
      const id = JSON.parse(scene.addSegmentation(JSON.stringify(specification)));
      const [index, generation] = id.split(":").map(BigInt);
      scene.bindSegmentation(index, generation, new Uint32Array(512).fill(7));
      identities.push({ id, index, generation });
    }
    const renderer = await wasm.Renderer.create(canvas, "interactive");
    const draw = () => renderer.renderCamera(scene,
      new Float32Array([0, 0, 45]), new Float32Array([0, 0, 0]), new Float32Array([0, 1, 0]));
    const pick = async x => JSON.parse((await renderer.pick(scene, x, 240)) ?? "null");
    draw();
    const initial = [await pick(250), await pick(390), await pick(320)];
    scene.resolve();
    scene.setSegmentStyles(identities[0].index, identities[0].generation,
      JSON.stringify([{ label: 7, color: [35, 220, 35, 255], opacity: 1, visible: true }]));
    draw();
    const restyled = [await pick(250), await pick(390)];
    scene.removeSegmentation(identities[0].index, identities[0].generation);
    scene.resolve();
    draw();
    const removed = [await pick(250), await pick(390)];
    renderer.free();
    scene.free();
    return { identities: identities.map(({ id }) => id), initial, restyled, removed };
  });
  expect(result.initial).toEqual([
    { pick: "volume_segment", segmentation: result.identities[0], volume_label: 7, label: "segment 7" },
    { pick: "volume_segment", segmentation: result.identities[1], volume_label: 7, label: "segment 7" },
    null,
  ]);
  expect(result.restyled.map(hit => hit.segmentation)).toEqual(result.identities);
  expect(result.removed).toEqual([null, result.initial[1]]);
  expect(errors).toEqual([]);
});
