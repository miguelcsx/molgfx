import { test, expect } from "@playwright/test";
test("measure real small, medium and large atom workloads", async ({ page }) => {
  const errors = []; page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const measurements = await page.evaluate(async () => {
    const { Viewer } = await import("/dist/index.js");
    const results = [];
    let queue, submits = 0;
    const submit = GPUQueue.prototype.submit;
    GPUQueue.prototype.submit = function (...args) { queue = this; submits++; return submit.apply(this, args); };
    try {
      for (const atoms of [1000, 10000, 50000]) {
        const source = Array.from({length: atoms}, (_, index) => {
          const coordinate = value => value.toFixed(3).padStart(8);
          return "ATOM  " + String(index + 1).padStart(5) + "  CA  ALA A" + String(index % 9999 + 1).padStart(4) + "    " + coordinate(index % 40 * 3) + coordinate(Math.floor(index / 40) % 40 * 3) + coordinate(Math.floor(index / 1600) * 3) + "  1.00 20.00           C";
        }).join("\n") + "\nEND\n";
        const start = performance.now();
        const viewer = await Viewer.create(document.querySelector("#viewer"));
        await viewer.load(new TextEncoder().encode(source), { name: "benchmark.pdb" });
        const loadMs = performance.now() - start;
        const commandStart = performance.now();
        viewer.show("spacefill"); viewer.color("orange"); viewer.focus();
        const commandMs = performance.now() - commandStart;
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        if (!queue) throw new Error("No real GPU submission");
        await queue.onSubmittedWorkDone();
        const readyMs = performance.now() - start;
        const cameraStart = performance.now(), cameraSubmits = submits;
        for (let i = 0; i < 12; i++) {
          document.querySelector("canvas").dispatchEvent(new KeyboardEvent("keydown", {key:"ArrowLeft",bubbles:true,cancelable:true}));
          await new Promise(resolve => requestAnimationFrame(resolve));
        }
        await queue.onSubmittedWorkDone();
        const cameraMs = performance.now() - cameraStart;
        const redrawSubmits = submits - cameraSubmits;
        await new Promise(resolve => setTimeout(resolve, 100));
        const idleStart = submits;
        await new Promise(resolve => setTimeout(resolve, 300));
        if (submits !== idleStart) throw new Error("Idle workload continues GPU submission");
        await viewer.dispose();
        results.push({atoms, load_ms: loadMs, command_ms: commandMs, gpu_ready_ms: readyMs, camera_12_frames_ms: cameraMs, camera_submissions: redrawSubmits, idle_submissions_300ms: submits - idleStart});
      }
    } finally { GPUQueue.prototype.submit = submit; }
    return results;
  });
  console.log("real workload measurements", JSON.stringify(measurements));
  expect(measurements.map(value => value.atoms)).toEqual([1000,10000,50000]);
  expect(errors).toEqual([]);
});
