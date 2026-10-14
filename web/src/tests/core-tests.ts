import { CleanupBag } from "../core/lifecycle.js";
import { SceneGeneration } from "../core/interaction.js";
import { readDeviceLoss, recoverRenderer } from "../render/device-loss.js";
import { RenderLoop } from "../render/render-loop.js";
import { PAGE_ELEMENTS, applyScenePatch, buildScene, patchChangesCamera } from "../runtime/scene.js";
import { AnywidgetAdapter } from "../anywidget/adapter.js";
import type { RuntimeModule, WasmCamera, WasmRenderer, WasmScene } from "../core/types.js";
import type { WidgetModel, WidgetTraits, ModelChangeEvent } from "../anywidget/model.js";
function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
const tests: Array<[string, () => void | Promise<void>]> = [];
function test(name: string, run: () => void | Promise<void>): void { tests.push([name, run]); }
test("Cleanup is reverse ordered, idempotent and immediate after disposal", () => {
  const seen: number[] = [], bag = new CleanupBag(); bag.add(() => seen.push(1)); bag.add(() => seen.push(2));
  bag.dispose(); bag.dispose(); bag.add(() => seen.push(3)); check(seen.join() === "2,1,3", "cleanup ownership/order");
});
test("Generation rejects every earlier scene identity", () => {
  const epoch = new SceneGeneration(); const old = epoch.value; epoch.advance(); check(!epoch.isCurrent(old), "stale generation accepted");
});
let frameId = 0;
const frames = new Map<number, FrameRequestCallback>();
Object.assign(globalThis, { window: { devicePixelRatio: 1 }, requestAnimationFrame: (callback: FrameRequestCallback) => { const id = ++frameId; frames.set(id, callback); return id; }, cancelAnimationFrame: (id: number) => frames.delete(id) });
function fixture() {
  const canvas = { width: 100, height: 100, clientWidth: 100, clientHeight: 100 } as HTMLCanvasElement;
  let draws = 0, finishes = 0, frees = 0, resolve!: (bytes: Uint8Array) => void;
  const pending = new Promise<Uint8Array>((done) => { resolve = done; });
  const camera = { position: new Float32Array([0, 0, 20]), target: new Float32Array(3), up: new Float32Array([0, 1, 0]), free() {} } as unknown as WasmCamera;
  const scene = { isReady: true, camera: () => camera } as unknown as WasmScene;
  const renderer = { renderCamera: () => { draws++; return false; }, resize() {}, beginPick: () => ({ resolve: () => pending, free: () => { frees++; } }), finishPick: () => { finishes++; return '{"pick":"atom","atom_index":3}'; } } as unknown as WasmRenderer;
  const loop = new RenderLoop(canvas, renderer, scene, { onSuccess() {}, onError(error) { throw error; } });
  return { loop, canvas, resolve, counts: () => ({ draws, finishes, frees }) };
}
for (const invalidate of ["camera", "resize", "scene", "dispose"] as const) test("Detached pick is rejected after " + invalidate + " changes the view", async () => {
  const f = fixture(); f.loop.drawNow(); const pending = f.loop.pick(20, 20); await Promise.resolve();
  if (invalidate === "camera") f.loop.invalidateCamera();
  if (invalidate === "resize") { Object.defineProperty(f.canvas, "clientWidth", { value: 140 }); f.loop.markSizeDirty(); }
  if (invalidate === "scene") f.loop.invalidatePicks();
  if (invalidate === "dispose") f.loop.dispose();
  if (invalidate !== "dispose") { const before = f.counts().draws; f.loop.drawNow(); check(f.counts().draws === before + 1, "readback incorrectly gates drawing"); }
  f.resolve(new Uint8Array(4)); const result = await pending;
  check(!result.performed && f.counts().finishes === 0 && f.counts().frees === 1, "stale result finished or readback leaked");
  f.loop.dispose(); await f.loop.settled(); check(frames.size === 0, "RAF remains after disposal");
});
test("Queued picks do not submit after disposal and hover callers settle", async () => {
  const f = fixture(); f.loop.drawNow(); const first = f.loop.pick(1, 1); await Promise.resolve();
  const second = f.loop.pick(2, 2), hover = f.loop.pickHover(3, 3); f.loop.dispose(); f.resolve(new Uint8Array(4));
  check(!(await first).performed && !(await second).performed && !(await hover).performed, "post-disposal pick publication");
  await f.loop.settled(); check(f.counts().frees === 1 && frames.size === 0, "queued readback/RAF leak");
});
test("Patch revision conflicts are refused and every patch is freed", () => {
  let freed = 0, applied = 0;
  class Patch { baseRevision: bigint; constructor(source: string) { this.baseRevision = BigInt(JSON.parse(source).base_revision as number); } free() { freed++; } }
  const runtime = { ScenePatch: Patch } as unknown as RuntimeModule;
  const scene = { revision: 9n, apply() { applied++; } } as unknown as WasmScene;
  check(!applyScenePatch(scene, '{"base_revision":8}', runtime).applied && applied === 0 && freed === 1, "conflict mutates or leaks");
  check(applyScenePatch(scene, '{"base_revision":9}', runtime).applied && Number(applied) === 1 && Number(freed) === 2, "matching patch was not applied/freed");
  check(patchChangesCamera({ operations: [{ op: "set_focus" }] }), "focus did not reframe");
});
test("Scene construction frees partial bindings on failure", () => {
  let freed = 0;
  class Scene { bindStructure() { throw new Error("bad payload"); } resolve() {} free() { freed++; } }
  const runtime = { Scene } as unknown as RuntimeModule;
  let rejected = false;
  try { buildScene({ spec: '{}', structures: [{ id: 1n, name: 'bad', payload: new Uint8Array() }] }, runtime); } catch { rejected = true; }
  check(rejected && freed === 1, "failed scene leaks");
});
test("Large columns cross the wasm boundary in bounded pages and free their accumulator", () => {
  const pushed: number[] = []; let freed = 0, whole = 0, paged = 0, total = 0;
  class FloatPages { constructor(n: number) { total = n; } push(page: Float32Array) { pushed.push(page.length); } free() { freed++; } }
  class Scene { restoreVolume() { whole++; } restoreVolumePaged() { paged++; } resolve() {} free() {} }
  const runtime = { Scene, FloatPages } as unknown as RuntimeModule;
  const big = new Uint8Array((PAGE_ELEMENTS * 2 + 5) * 4), small = new Uint8Array(16);
  const volume = (values: Uint8Array) => ({ id: 1n, values } as never);
  buildScene({ spec: '{}', structures: [], bindings: { volumes: [volume(big), volume(small)] } } as never, runtime);
  check(whole === 1 && paged === 1 && freed === 1, "small column paged or accumulator leaked");
  check(total === PAGE_ELEMENTS * 2 + 5 && pushed.join() === [PAGE_ELEMENTS, PAGE_ELEMENTS, 5].join(), "pages unbounded or out of order");
});
test("Anywidget preserves exact pick identity and unsubscribes its transport", () => {
  const values: Partial<WidgetTraits> = { scene_spec: '{}', revision: 4, structure_ids: [1, 2], structure_names: ['a','b'], structure_payloads: [new Uint8Array([9]), new Uint8Array()], structure_sources: [1, 1] };
  const watching = new Map<ModelChangeEvent, () => void>();
  const model = { get: (key: keyof WidgetTraits) => values[key], set: (key: keyof WidgetTraits, value: unknown) => { Object.assign(values, { [key]: value }); }, save_changes() {}, on: (key: ModelChangeEvent, fn: () => void) => watching.set(key, fn), off: (key: ModelChangeEvent) => watching.delete(key) } as WidgetModel;
  const adapter = new AnywidgetAdapter(model), snapshot = adapter.snapshot();
  check(snapshot.structures[0]?.payload === snapshot.structures[1]?.payload, "aliased structure bytes copied");
  const stop = adapter.onPatch(() => {}); stop(); check(watching.size === 0, "trait subscription leak");
  adapter.publishInteraction({ kind: 'pick', result: '{"pick":"atom","structure":2,"dataset":8,"atom_index":7}', eventId: '1', modifiers: { shift:false, control:false, meta:false, alt:false }, sceneGeneration: 1 });
  check(values.pick?.structure === 2 && values.pick.atom_index === 7 && values.scene_spec === '{}', "pick flattened or implicitly selected");
});
const lostJSON = '{"reason":"destroyed","recoverable":true,"message":"gone"}';
function lossy(json: string | undefined) { let freed = 0; return { renderer: { deviceLossJSON: () => json, free: () => { freed++; } } as unknown as WasmRenderer, freed: () => freed }; }
test("Device loss JSON is parsed into a typed state and healthy renderers report none", () => {
  const loss = readDeviceLoss(lossy(lostJSON).renderer);
  check(loss?.reason === "destroyed" && loss.recoverable && loss.message === "gone", "loss not typed");
  check(readDeviceLoss(lossy(undefined).renderer) === undefined, "healthy renderer reported loss");
});
test("Recoverable loss swaps in a fresh renderer and frees the lost one after settling", async () => {
  const old = lossy(lostJSON), next = lossy(undefined); const order: string[] = [];
  const loop = { replaceRenderer: () => { order.push("swap"); }, settled: async () => { order.push("settled"); } };
  const loss = readDeviceLoss(old.renderer)!;
  const result = await recoverRenderer({ loss, current: old.renderer, loop, create: async () => next.renderer, isDisposed: () => false });
  check(result.recreated && result.renderer === next.renderer && old.freed() === 1 && next.freed() === 0 && order.join() === "swap,settled", "recreate path wrong");
});
test("Unrecoverable loss or disposal never leaks or swaps renderers", async () => {
  const old = lossy(lostJSON), next = lossy(undefined); let swaps = 0;
  const loop = { replaceRenderer: () => { swaps++; }, settled: async () => {} };
  const base = { current: old.renderer, loop, create: async () => next.renderer };
  const fatal = await recoverRenderer({ ...base, loss: { reason: "unknown", recoverable: false, message: "" }, isDisposed: () => false });
  check(!fatal.recreated && fatal.renderer === old.renderer && old.freed() === 0, "fatal loss swapped");
  const gone = await recoverRenderer({ ...base, loss: readDeviceLoss(old.renderer)!, isDisposed: () => true });
  check(!gone.recreated && next.freed() === 1 && swaps === 0 && old.freed() === 0, "disposed recovery leaked");
});
for (const [name, run] of tests) { await run(); console.log("PASS " + name); }
console.log(tests.length + " lifecycle/revision tests passed");
