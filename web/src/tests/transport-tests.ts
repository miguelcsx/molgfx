import type { TimeChange, TransportClock, TransportLoopMode } from "../core/contracts.js";
import { Transport } from "../runtime/transport.js";
function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
const tests: Array<[string, () => void]> = [];
function test(name: string, run: () => void): void { tests.push([name, run]); }
/** Manual clock: `step(ms)` advances time and fires the pending callback, if any. */
function fixture(mode: TransportLoopMode, duration = 10, rate = 1) {
  let now = 0, next = 0; const pending = new Map<number, (t: number) => void>();
  const clock: TransportClock = { request: (cb) => { pending.set(++next, cb); return next; }, cancel: (h) => { pending.delete(h); } };
  const applied: number[] = []; let readingsBuffer: Float64Array | undefined, reallocated = false;
  const timeline = { trackCount: 2, apply: (_scene: never, s: number) => { applied.push(s); }, readingsInto: (out: Float64Array) => {
    if (readingsBuffer && readingsBuffer !== out) reallocated = true; readingsBuffer = out;
    out.set([0, applied.at(-1) ?? 0, 0.5, 1, 1, 2e-9, 0.25, 2]); return 2; } };
  const transport = new Transport({ timeline, layout: { readingStride: 4, unitSymbols: ["as", "fs", "ps", "ns", "us", "ms", "s"] }, scene: undefined as never, clock, durationSeconds: duration, loopMode: mode, rate });
  const step = (ms: number) => { now += ms; const callbacks = [...pending.values()]; pending.clear(); for (const cb of callbacks) cb(now); };
  return { transport, applied, step, pending: () => pending.size, reallocated: () => reallocated };
}
test("Seek clamps to the presentation range", () => {
  const f = fixture("clamp"); f.transport.seek(-5); check(f.applied[0] === 0, "low clamp");
  f.transport.seek(99); check(f.transport.playbackSeconds === 10 && f.applied.join() === "0,10", "high clamp");
});
test("Clamp stops playback at the end", () => {
  const f = fixture("clamp", 1); const states: boolean[] = []; f.transport.on("playingchange", (p) => states.push(p));
  f.transport.play(); f.step(0); f.step(600); f.step(600);
  check(f.transport.playbackSeconds === 1 && !f.transport.playing && f.pending() === 0 && states.join() === "true,false", "clamp end");
});
test("Loop wraps past the end and below zero", () => {
  const f = fixture("loop"); f.transport.seek(9); f.transport.play(); f.step(0); f.step(2000);
  check(Math.abs(f.transport.playbackSeconds - 1) < 1e-9, "forward wrap");
  f.transport.rate = -1; f.step(2000); check(Math.abs(f.transport.playbackSeconds - 9) < 1e-9, "backward wrap");
});
test("Pingpong reverses at both ends", () => {
  const f = fixture("pingpong"); f.transport.seek(9); f.transport.play(); f.step(0); f.step(2000);
  check(Math.abs(f.transport.playbackSeconds - 9) < 1e-9, "reflects at the end");
  f.step(9000); check(Math.abs(f.transport.playbackSeconds - 0) < 1e-9, "reaches the start");
  f.step(1500); check(Math.abs(f.transport.playbackSeconds - 1.5) < 1e-9, "reflects at the start");
});
test("A zero rate holds position and applies nothing", () => {
  const f = fixture("loop", 10, 0); f.transport.play(); f.step(0); f.step(1000); f.step(1000);
  check(f.transport.playbackSeconds === 0 && f.applied.length === 0 && f.transport.playing, "zero rate moved or applied");
});
test("Pause cancels the tick and nothing applies while paused", () => {
  const f = fixture("loop"); f.transport.play(); f.step(0); f.step(100); const count = f.applied.length;
  f.transport.pause(); check(f.pending() === 0, "tick still scheduled"); f.step(1000); f.step(1000);
  check(f.applied.length === count && count === 1, "apply ran while paused");
});
test("timechange carries playback seconds and typed readouts", () => {
  const f = fixture("clamp"); const seen: Array<{ s: number; unit: string; value: number; n: number }> = [];
  f.transport.on("timechange", (e: TimeChange) => seen.push({ s: e.playbackSeconds, unit: e.readouts[0]?.unit ?? "", value: e.readouts[0]?.value ?? 0, n: e.readouts.length }));
  f.transport.seek(3); const e = seen[0]; check(e && e.s === 3 && e.n === 2 && e.unit === "fs", "payload shape");
  check(e.value === 3 / 1e-15, "display value in unit");
});
test("The reading buffer and payload are reused across ticks", () => {
  const f = fixture("loop"); const payloads = new Set<object>(); f.transport.on("timechange", (e) => payloads.add(e));
  f.transport.play(); f.step(0); for (let i = 0; i < 5; i++) f.step(16);
  check(!f.reallocated() && payloads.size === 1 && f.applied.length === 5, "tick allocated per frame");
});
for (const [name, run] of tests) { run(); console.log("PASS " + name); }
