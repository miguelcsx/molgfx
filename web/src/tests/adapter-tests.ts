import { AnywidgetAdapter } from "../anywidget/adapter.js";
import type { WidgetModel, WidgetTraits } from "../anywidget/model.js";
import type { CoordinateFrame, TransportClock } from "../core/contracts.js";
import { Transport } from "../runtime/transport.js";
import { CHUNK_FRAMES, CHUNK_WINDOW_FRAMES, ChunkWindow } from "../runtime/trajectory-chunks.js";
function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
const tests: Array<[string, () => void]> = [];
function test(name: string, run: () => void): void { tests.push([name, run]); }
function harness() {
  const sent: unknown[] = []; let onMessage: ((content: unknown) => void) | undefined; const traits: Partial<WidgetTraits> = {}; const errors: string[] = [];
  const model = { get: (key: keyof WidgetTraits) => traits[key], set: (key: keyof WidgetTraits, value: unknown) => { Object.assign(traits, { [key]: value }); errors.push(String(value)); }, save_changes() {}, send: (content: unknown) => { sent.push(content); },
    on: (event: string, fn: (content: unknown) => void) => { if (event === "msg:custom") onMessage = fn; }, off: (event: string) => { if (event === "msg:custom") onMessage = undefined; } } as unknown as WidgetModel;
  let next = 0; const pending = new Map<number, (t: number) => void>();
  const clock: TransportClock = { request: (cb) => { pending.set(++next, cb); return next; }, cancel: (h) => { pending.delete(h); } };
  const applied: number[] = []; const raw: Float64Array[] = [];
  const timeline = { trackCount: 1, apply: (_s: never, s: number) => { applied.push(s); }, readingsInto: (out: Float64Array) => { raw.push(out); out.set([0, 2e-9, 0.5, 3]); return 1; } };
  const transport = new Transport({ timeline, layout: { readingStride: 4, unitSymbols: ["as", "fs", "ps", "ns", "us", "ms", "s"] }, scene: undefined as never, clock, durationSeconds: 10 });
  const adapter = new AnywidgetAdapter(model); const stop = adapter.bindTransport(transport);
  return { adapter, transport, sent, send: (content: unknown) => onMessage?.(content), stop, bound: () => onMessage !== undefined, errors, pending: () => pending.size };
}
test("Each kernel message is forwarded to the transport", () => {
  const h = harness();
  h.send({ type: "seek", seconds: 4 }); check(h.transport.playbackSeconds === 4, "seek");
  h.send({ type: "set_rate", rate: -2 }); check(h.transport.rate === -2, "rate");
  h.send({ type: "set_loop", mode: "pingpong" }); check(h.transport.loopMode === "pingpong", "loop");
  h.send({ type: "play" }); check(h.transport.playing && h.pending() === 1, "play");
  h.send({ type: "pause" }); check(!h.transport.playing && h.pending() === 0, "pause");
});
test("Malformed commands are reported and never reach the transport", () => {
  const h = harness();
  h.send({ type: "seek", seconds: "4" }); h.send({ type: "set_rate", rate: Infinity }); h.send({ type: "set_loop", mode: "ping_pong" });
  check(h.transport.playbackSeconds === 0 && h.transport.rate === 1 && h.transport.loopMode === "clamp", "bad value applied");
  check(h.errors.length === 3, "errors not reported");
});
test("timechange carries seconds and flat readouts without reallocating", () => {
  const h = harness(); h.send({ type: "seek", seconds: 1 }); h.send({ type: "seek", seconds: 2 });
  const [a, b] = h.sent as Array<{ type: string; seconds: number; readouts: Array<number | string> }>;
  check(a && b && a === b, "message object not reused");
  check(b.type === "timechange" && b.seconds === 2 && b.readouts.join() === "0,2,ns,0.5", "payload " + String(b?.readouts));
});
test("Unbinding stops forwarding and time reports", () => {
  const h = harness(); h.stop(); check(!h.bound(), "listener remains"); h.transport.seek(3); check(h.sent.length === 0, "sent after unbind");
});
const frame = (index: number): CoordinateFrame => ({ id: BigInt(index + 100), index: BigInt(index), seconds: index, positions: new Uint8Array(12) });
const run = (from: number, count: number) => Array.from({ length: count }, (_, i) => frame(from + i));
test("Chunk window brackets a time and prefetches before it runs out", () => {
  const w = new ChunkWindow(); const first = w.next(0.5); check(first?.count === CHUNK_FRAMES && w.next(0.5) === undefined, "one request in flight");
  w.accept(run(0, CHUNK_FRAMES), 0.5); const pair = w.pairAt(0.5); check(pair?.start.index === 0n && pair.end.index === 1n, "bracket");
  const ahead = w.next(CHUNK_FRAMES - 2.5); check(ahead?.fromSeconds === CHUNK_FRAMES - 1, "prefetch from window end");
  w.accept(run(CHUNK_FRAMES - 1, CHUNK_FRAMES), CHUNK_FRAMES - 2.5); check(w.size === CHUNK_FRAMES * 2 - 1, "overlap deduplicated");
});
test("Chunk window stays bounded and marks the end of a trajectory", () => {
  const w = new ChunkWindow(); w.next(0); w.accept(run(0, CHUNK_FRAMES), 0);
  for (let at = CHUNK_FRAMES; at < 10 * CHUNK_FRAMES; at += CHUNK_FRAMES - 1) { const s = at - CHUNK_FRAMES + 0.5; w.next(s); w.accept(run(at - 1, CHUNK_FRAMES), s); check(w.size <= CHUNK_WINDOW_FRAMES, "window grew"); }
  w.next(1000); w.accept([], 1000); check(w.next(1001) === undefined, "asked past the end");
});
test("A jump outside the window replaces it", () => {
  const w = new ChunkWindow(); w.next(0); w.accept(run(0, CHUNK_FRAMES), 0); const jump = w.next(500); check(jump?.fromSeconds === 500, "jump request");
  w.accept(run(500, CHUNK_FRAMES), 500); check(w.size === CHUNK_FRAMES && w.pairAt(500.5)?.start.index === 500n && w.pairAt(0.5) === undefined, "not replaced");
});
for (const [name, body] of tests) { body(); console.log("PASS " + name); }
