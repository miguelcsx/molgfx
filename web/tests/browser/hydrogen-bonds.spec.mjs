import { test, expect } from "@playwright/test";
import { screenshotColors } from "./pixels.mjs";

test("oriented candidates retain binding atomicity and reject distant or bent explicit hydrogens", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const coverage = [];
  for (const [hydrogen, acceptor] of [["1.000   0.000", "2.800"], ["1.000   0.000", "6.000"], ["1.000   1.000", "2.800"]]) {
    const result = await page.evaluate(async ([hydrogen, acceptor]) => {
      const wasm = await import("/dist/molgfx_wasm.js");
      await wasm.default({ module_or_path: "/dist/molgfx_wasm_bg.wasm" });
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 256;
      document.querySelector("#viewer").append(canvas);
      const pdb = `ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 20.00           N\nATOM      2  H   ALA A   1       ${hydrogen}   0.000  1.00 20.00           H\nATOM      3  O   ALA A   2       ${acceptor}   0.000   0.000  1.00 20.00           O\nEND\n`;
      const scene = wasm.Scene.fromStructureBytes(new TextEncoder().encode(pdb), "explicit.pdb");
      const interaction = { mode: "hydrogen_bonds", source: { content_hash: "motion-candidates", uri: null, format: null },
        structure: 1, selection: "not element H", width_pixels: 3,
        color: [0, 120, 255, 255], opacity: 1,
        geometry: { maximum_endpoint_distance: 3.5, minimum_angle_degrees: 150,
          maximum_pivot_end_distance: 2.5 } };
      const patch = new wasm.ScenePatch(JSON.stringify({ base_revision: Number(scene.revision),
        operations: [{ op: "add_interaction", id: 1, interaction }] }));
      scene.apply(patch); patch.free();
      scene.resolve();
      const revision = scene.revision;
      let refused = false;
      try { scene.bindHydrogenBonds(1n, new Uint32Array([0, 1, 2]), 2); } catch { refused = true; }
      const unchanged = scene.revision === revision;
      const pending = JSON.parse(scene.unresolvedOverlays());
      scene.bindHydrogenBonds(1n, new Uint32Array([0, 1, 2]), 3.5);
      scene.resolve();
      const renderer = await wasm.Renderer.create(canvas, "interactive");
      const presented = renderer.renderCamera(scene, new Float32Array([1.4, 0, 10]),
        new Float32Array([1.4, 0, 0]), new Float32Array([0, 1, 0]));
      window.hydrogenProof = { renderer, scene, canvas };
      return { refused, unchanged, pending, presented, unresolved: JSON.parse(scene.unresolvedOverlays()) };
    }, [hydrogen, acceptor]);
    expect(result).toEqual({ refused: true, unchanged: true, pending: [{ kind: "interaction", id: 1 }], presented: true, unresolved: [] });
    coverage.push((await screenshotColors(page)).blue);
    await page.evaluate(() => {
      window.hydrogenProof.renderer.free(); window.hydrogenProof.scene.free(); window.hydrogenProof.canvas.remove();
    });
  }
  expect(coverage[0]).toBeGreaterThan(50);
  expect(coverage.slice(1)).toEqual([0, 0]);
  expect(errors).toEqual([]);
});
