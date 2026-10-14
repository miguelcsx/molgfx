import type { SequenceManifest, SequenceOptions, TransportClock } from "../core/contracts.js";
import type { WasmCamera, WasmRenderer, WasmScene } from "../core/types.js";
import { RenderLoop } from "../render/render-loop.js";
import { captureFrameSequence } from "../runtime/frame-sequence.js";
import { Transport } from "../runtime/transport.js";
function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
const tests: Array<[string, () => Promise<void>]> = [];
function test(name: string, run: () => Promise<void>): void { tests.push([name, run]); }
let rafCalls = 0;
Object.assign(globalThis, { window: { devicePixelRatio: 1 }, requestAnimationFrame: () => { rafCalls += 1; return rafCalls; }, cancelAnimationFrame: () => {} });
/** A fake renderer, scene, canvas and clock around the real transport and render loop. */
function fixture(duration = 4, accumulate = 0) {
  const canvas = { width: 1, height: 1, clientWidth: 8, clientHeight: 8, toBlob: (done: (b: Blob | null) => void) => { encoded.push(canvas.width + "x" + canvas.height); done(new Blob(["png"])); } };
  const encoded: string[] = [], applied: number[] = [], sizes: string[] = [];
  let failAt = -1, failure: unknown, draws = 0, buffer: Float64Array | undefined, reallocated = false, pendingPasses = 0;
  const renderer = { renderCamera: () => { draws += 1; if (pendingPasses > 0) { pendingPasses -= 1; return true; } return false; }, resize: (w: number, h: number) => { sizes.push(w + "x" + h); } } as unknown as WasmRenderer;
  const camera = { position: new Float32Array(3), target: new Float32Array(3), up: new Float32Array(3), free() {} } as unknown as WasmCamera;
  const scene = { isReady: true, camera: () => camera } as unknown as WasmScene;
  const loop = new RenderLoop(canvas as unknown as HTMLCanvasElement, renderer, scene, { onError: (e) => { throw e; }, onSuccess() {} });
  const clock: TransportClock = { request: () => { throw new Error("the transport clock must not run"); }, cancel() {} };
  const timeline = { trackCount: 2, apply: (_s: never, seconds: number) => { if (applied.length === failAt) { failure = new Error("apply broke"); return; } applied.push(seconds); pendingPasses = accumulate; loop.requestFrame(); },
    readingsInto: (out: Float64Array) => { if (buffer && buffer !== out) reallocated = true; buffer = out; out.set([0, applied.at(-1) ?? 0, 0, 1, 1, 3e-9, 0, 3]); return 2; } };
  const transport = new Transport({ timeline, layout: { readingStride: 4, unitSymbols: ["as", "fs", "ps", "ns", "us", "ms", "s"] }, scene: undefined as never, clock, durationSeconds: duration });
  const surface = { begin: (w: number, h: number) => loop.beginCapture(w, h), settle: () => loop.drawConverged(8), takeError: () => { const e = failure; failure = undefined; return e; }, end: () => loop.endCapture(),
    encode: () => new Promise<Blob>((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("encode")))))};
  return { fail: (k: number) => { failAt = k; }, transport, surface, applied, encoded, sizes, draws: () => draws, reallocated: () => reallocated, canvas };
}
function options(extra: Partial<SequenceOptions> & { onFrame?: SequenceOptions["onFrame"] }): SequenceOptions {
  return { fps: 60, size: { width: 16, height: 12 }, onFrame: () => {}, ...extra };
}
for (const fps of [60, 24]) test(`Frame i is captured at exactly i divided by fps at ${fps} fps`, async () => {
  const f = fixture(2); const seen: Array<[number, number, number]> = [];
  const manifest = await captureFrameSequence(f.transport, f.surface, options({ fps, seconds: 2, onFrame: (frame) => { seen.push([frame.index, frame.playbackSeconds, frame.readouts.length]); } }));
  check(seen.length === 2 * fps && manifest.frameCount === 2 * fps && manifest.completed, "frame count");
  for (let i = 0; i < seen.length; i++) {
    check(seen[i]![0] === i && seen[i]![1] === i / fps && f.applied[i] === i / fps, "frame " + i + " time");
    check(manifest.frames[i]!.playbackSeconds === i / fps && manifest.fps === fps, "manifest time " + i);
  }
  const last = manifest.frames.at(-1)!; check(last.tracks[0]!.simSeconds === last.playbackSeconds && last.tracks[0]!.unit === "fs" && last.tracks[1]!.unit === "ns", "track times");
  check(!JSON.stringify(manifest).includes("version") && manifest.interpolation.includes("linear interpolation"), "manifest shape");
});
test("No animation frame is requested while capturing and the canvas is restored after", async () => {
  const f = fixture(1); const before = rafCalls;
  await captureFrameSequence(f.transport, f.surface, options({ fps: 10 }));
  check(f.applied.length === 10 && rafCalls === before + 1, "rAF during capture: " + (rafCalls - before)); // one: endCapture restores on-demand drawing
  check(f.encoded.every((s) => s === "16x12") && f.sizes.join() === "16x12", "frames were not the requested size");
});
test("Each frame renders to convergence and the readout buffer is reused", async () => {
  const f = fixture(1, 3); await captureFrameSequence(f.transport, f.surface, options({ fps: 4 }));
  check(f.draws() === 4 * 4, "passes: " + f.draws()); check(!f.reallocated(), "readout buffer reallocated");
});
test("Frames are handed over one at a time and the loop waits for onFrame", async () => {
  const f = fixture(1); let inFlight = 0, peak = 0; const frames = new Set<object>();
  await captureFrameSequence(f.transport, f.surface, options({ fps: 5, onFrame: async (frame) => { frames.add(frame); inFlight += 1; peak = Math.max(peak, inFlight); await new Promise((r) => setTimeout(r, 1)); inFlight -= 1; } }));
  check(peak === 1 && frames.size === 1, "overlap or per-frame allocation");
});
test("Abort stops between frames and leaves the transport paused", async () => {
  const f = fixture(2); const controller = new AbortController(); let delivered = 0;
  const manifest: SequenceManifest = await captureFrameSequence(f.transport, f.surface, options({ fps: 10, signal: controller.signal, onFrame: () => { delivered += 1; if (delivered === 3) controller.abort(); } }));
  check(delivered === 3 && manifest.frameCount === 3 && !manifest.completed && manifest.frames.length === 3, "stopped late or early");
  check(!f.transport.playing && f.applied.length === 3, "transport moved after abort");
});
test("Invalid options are refused before any frame is applied", async () => {
  const f = fixture(1);
  for (const bad of [{ fps: 0 }, { fps: 2.5 }, { size: { width: 0, height: 4 } }, { seconds: 5 }]) {
    let refused = false; try { await captureFrameSequence(f.transport, f.surface, options(bad)); } catch (e) { refused = e instanceof RangeError; }
    check(refused, "accepted " + JSON.stringify(bad));
  }
  check(f.applied.length === 0, "applied before validation");
});
test("An apply error on frame k stops the capture and names frame k", async () => {
  const f = fixture(2); f.fail(4); const seen: number[] = []; let message = "";
  try { await captureFrameSequence(f.transport, f.surface, options({ fps: 10, onFrame: (frame) => { seen.push(frame.index); } })); } catch (e) { message = e instanceof Error ? e.message : String(e); }
  check(message.includes("frame 4") && message.includes("apply broke"), "error: " + message);
  check(seen.join() === "0,1,2,3" && !f.transport.playing, "frames after the failure: " + seen.join());
});
test("A frame that never converges is delivered marked and counted", async () => {
  const f = fixture(1, Number.POSITIVE_INFINITY); const flags: boolean[] = [];
  const manifest = await captureFrameSequence(f.transport, f.surface, options({ fps: 3, onFrame: () => { flags.push(true); } }));
  check(flags.length === 3 && manifest.completed && manifest.frames.every((x) => !x.converged) && manifest.unconvergedFrames === 3, "unconverged marking");
  const g = fixture(1); const ok = await captureFrameSequence(g.transport, g.surface, options({ fps: 3 }));
  check(ok.unconvergedFrames === 0 && ok.frames.every((x) => x.converged), "converged frames were marked");
});
for (const [name, run] of tests) { await run(); console.log("PASS " + name); }
