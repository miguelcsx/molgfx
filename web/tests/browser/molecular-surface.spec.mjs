import { test, expect } from "@playwright/test";
import { screenshotColors } from "./pixels.mjs";

test("transported molecular triangles survive resolution and wire edits without consuming failed bindings", async ({ page }) => {
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const state = await page.evaluate(async () => {
    const wasm = await import("/dist/molgfx_wasm.js");
    await wasm.default({ module_or_path: "/dist/molgfx_wasm_bg.wasm" });
    const canvas = document.createElement("canvas");
    canvas.width = 640; canvas.height = 480;
    document.querySelector("#viewer").append(canvas);
    const bytes = new TextEncoder().encode("HETATM    1  O   HOH A   1       0.000   0.000   0.000  1.00 10.00           O\nEND\n");
    const scene = wasm.Scene.fromStructureBytes(bytes, "oxygen.pdb");
    const representation = {
      common: { structure: 1, target: "all", color: { kind: "element" }, opacity: 1,
        visual: null, parameters: {}, visible: true },
      form: { kind: "molecular_surface", source: { content_hash: "surface-output", uri: null, format: null },
        method: "msms", controls: { probe_radius: 1.4, density: 3, all_components: true }, wire: false, width: 1.5 },
    };
    function patch(operations) {
      const value = new wasm.ScenePatch(JSON.stringify({ base_revision: Number(scene.revision), operations }));
      try { scene.apply(value); } finally { value.free(); }
    }
    patch([{ op: "add_representation", id: 1, representation }]);
    const data = { method: "surf", controls: representation.form.controls,
      mesh: { vertices: [[-1.5, -1.5, 0], [1.5, -1.5, 0], [0, 1.5, 0]],
        normals: [[0, 0, 1], [0, 0, 1], [0, 0, 1]], faces: [[0, 1, 2]], atom_rows: [0, 0, 0] } };
    let refused = false;
    const revision = scene.revision;
    try { scene.bindMolecularSurface(1n, JSON.stringify(data)); } catch { refused = true; }
    const unchanged = revision === scene.revision;
    const pending = JSON.parse(scene.unresolvedRepresentations());
    data.method = "msms";
    scene.bindMolecularSurface(1n, JSON.stringify(data));
    scene.resolve();
    const renderer = await wasm.Renderer.create(canvas, "interactive");
    function render() {
      return renderer.renderCamera(scene, new Float32Array([0, 0, 12]),
        new Float32Array([0, 0, 0]), new Float32Array([0, 1, 0]));
    }
    const presented = render();
    window.surfaceProof = { renderer, scene, representation, patch, render };
    return { refused, unchanged, pending, presented, unresolved: JSON.parse(scene.unresolvedRepresentations()) };
  });
  const solid = await screenshotColors(page);
  await page.evaluate(() => {
    const proof = window.surfaceProof;
    proof.representation.form.wire = true;
    proof.patch([{ op: "replace_representation", id: 1, representation: proof.representation }]);
    proof.scene.resolve();
    proof.render();
  });
  const wire = await screenshotColors(page);
  const removed = await page.evaluate(() => {
    const proof = window.surfaceProof;
    proof.patch([{ op: "remove_representation", id: 1 }]);
    proof.scene.resolve();
    const unresolved = JSON.parse(proof.scene.unresolvedRepresentations());
    proof.renderer.free(); proof.scene.free();
    return unresolved;
  });
  expect(state).toEqual({ refused: true, unchanged: true, pending: [1], presented: true, unresolved: [] });
  expect(solid.red).toBeGreaterThan(1000);
  expect(wire.red).toBeGreaterThan(100);
  expect(wire.red).toBeLessThan(solid.red / 2);
  expect(removed).toEqual([]);
  expect(errors).toEqual([]);
});
