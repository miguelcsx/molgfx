"""The Workbench console and the notebook runtime path, in a real browser.

A notebook frontend loads the widget module from a blob URL, so nothing beside
it can be imported relatively. These pages serve ``widget.js`` from a directory
that holds no runtime at all and hand the runtime over as widget state, exactly
as a kernel does, so a page that loads here loads in Jupyter and Colab.
"""

import base64
import gzip
import json
from pathlib import Path
import shutil
import tempfile
import unittest

import molgfx
from molgfx.viewer import Workbench
from molgfx.viewer._runtime import load_runtime

from test_viewer import PAGE, STATIC, _LocalPage, structure, sync_playwright


def _values(bench):
    """The synchronized state a frontend receives for ``bench``."""
    sources = bench.scene._browser_sources()
    runtime = load_runtime()
    return {
        "scene_spec": bench.scene.to_json(),
        "scene_patch": "",
        "patch_sequence": 0,
        "structure_ids": [source[0] for source in sources],
        "structure_names": [source[1] for source in sources],
        "structure_payloads": [
            base64.b64encode(bytes(source[2])).decode("ascii") for source in sources
        ],
        "pick": {},
        "selection": "",
        "camera": {},
        "error": "",
        "revision": bench.scene.revision,
        "sync_request": 0,
        "workbench": True,
        "history": [],
        "command_request": {},
        "interaction_event": {},
        "sequence_intervals": {},
        "focus_preset": "",
        "measurement_request": {},
        "volume_sigma": {},
        "trajectory_frame": {},
        "trajectory_time": {},
        "command_reply": {},
        "_runtime_js": runtime.glue,
        "_runtime_wasm": base64.b64encode(runtime.wasm_gzip).decode("ascii"),
        "_runtime_key": runtime.key,
    }


@unittest.skipUnless(load_runtime().key, "the browser runtime is not built")
class RuntimeTransportTests(unittest.TestCase):
    def test_the_runtime_travels_compressed_and_inflates_to_the_packaged_binary(self):
        data = load_runtime().wasm_gzip
        packaged = (STATIC / "molgfx_wasm_bg.wasm").read_bytes()
        self.assertEqual(gzip.decompress(data), packaged)
        self.assertLess(len(data), len(packaged) // 2)


@unittest.skipUnless(sync_playwright is not None, "playwright is not installed")
@unittest.skipUnless(load_runtime().key, "the browser runtime is not built")
class WorkbenchPageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls._directory = Path(tempfile.mkdtemp())
        # Only the module and its stylesheet: no runtime beside them.
        shutil.copy(STATIC / "widget.js", cls._directory / "widget.js")
        shutil.copy(STATIC / "widget.css", cls._directory / "widget.css")
        bench = Workbench(structure())
        bench.execute("show spacefill, all")
        page = PAGE.replace("__VALUES__", json.dumps(_values(bench))).replace(
            "window.__molgfx.values.structure_payloads = window.__molgfx.bytes(window.__molgfx.values.structure_payloads);",
            "window.__molgfx.values.structure_payloads = window.__molgfx.bytes(window.__molgfx.values.structure_payloads);\n"
            "window.__molgfx.values._runtime_wasm = window.__molgfx.bytes([window.__molgfx.values._runtime_wasm])[0];",
        )
        (cls._directory / "page.html").write_text(page)
        cls._playwright = sync_playwright().start()
        cls._local = _LocalPage(cls._directory).__enter__()
        cls._browser = cls._playwright.chromium.launch(
            args=["--enable-unsafe-webgpu", "--use-angle=swiftshader"]
        )

    @classmethod
    def tearDownClass(cls):
        cls._browser.close()
        cls._playwright.stop()
        cls._local.__exit__(None, None, None)
        shutil.rmtree(cls._directory, ignore_errors=True)

    def setUp(self):
        self.page = self._browser.new_page()
        self.page.goto(f"http://127.0.0.1:{self._local.port}/page.html")
        self.page.wait_for_function(
            "window.__molgfx.cleanup !== undefined || window.__molgfx.setupError !== ''"
        )

    def tearDown(self):
        self.page.close()

    def runtime(self):
        """The page's live widget state, skipping when the GPU runtime is absent."""
        setup_error = self.page.evaluate("window.__molgfx.setupError")
        if setup_error:
            self.skipTest(f"the browser runtime is unavailable: {setup_error}")
        return self.page.evaluate("window.__molgfx.values")

    def test_the_runtime_loads_from_widget_state_alone(self):
        exports = self.page.evaluate(
            """async () => {
                const {loadRuntime} = await import("./widget.js");
                const runtime = await loadRuntime(window.__molgfx.model);
                return ["Scene", "ScenePatch", "Renderer", "Session"].filter((name) => name in runtime);
            }"""
        )
        self.assertEqual(exports, ["Scene", "ScenePatch", "Renderer", "Session"])

    def test_a_view_into_a_larger_buffer_decodes_to_its_own_bytes(self):
        decoded = self.page.evaluate(
            """async () => {
                const {sourceBytes} = await import("./widget.js");
                const buffer = new Uint8Array([9, 9, 1, 2, 3, 9]).buffer;
                return [
                    Array.from(sourceBytes(new DataView(buffer, 2, 3))),
                    Array.from(sourceBytes(new Uint8Array(buffer, 2, 3))),
                    Array.from(sourceBytes(buffer)),
                ];
            }"""
        )
        self.assertEqual(decoded, [[1, 2, 3], [1, 2, 3], [9, 9, 1, 2, 3, 9]])

    def test_a_browser_without_webgpu_says_so_and_names_the_static_path(self):
        page = self._browser.new_page()
        try:
            page.add_init_script("delete Navigator.prototype.gpu;")
            page.goto(f"http://127.0.0.1:{self._local.port}/page.html")
            page.wait_for_function("window.__molgfx.setupError !== ''")
            message = page.evaluate("window.__molgfx.setupError")
            self.assertIn("does not expose WebGPU", message)
            self.assertIn("render_image", message)
            self.assertIn("does not expose WebGPU", page.inner_text(".molgfx-failure"))
            self.assertIn("does not expose WebGPU", page.evaluate("window.__molgfx.values.error"))
        finally:
            page.close()

    def test_a_frame_that_fails_is_reported_under_the_canvas(self):
        self.assertTrue(self.page.locator(".molgfx-canvas + .molgfx-failure").is_hidden())
        self.page.evaluate(
            """async () => {
                const {loadRuntime} = await import("./widget.js");
                const runtime = await loadRuntime(window.__molgfx.model);
                runtime.Renderer.prototype.renderCamera = () => {
                    throw new Error("the GPU rejected a shader");
                };
                document.getElementById("root").style.width = "70%";
            }"""
        )
        notice = self.page.locator(".molgfx-canvas + .molgfx-failure")
        notice.wait_for(state="visible")
        self.assertIn("the GPU rejected a shader", notice.inner_text())
        self.assertIn("the GPU rejected a shader", self.page.evaluate("window.__molgfx.values.error"))

    def test_a_high_density_canvas_renders_within_the_pixel_budget(self):
        sizes = self.page.evaluate(
            """async () => {
                const {dimensions, PIXEL_BUDGET} = await import("./widget.js");
                Object.defineProperty(window, "devicePixelRatio", {value: 2, configurable: true});
                return {
                    budget: PIXEL_BUDGET,
                    wide: dimensions({clientWidth: 1960, clientHeight: 980}),
                    small: dimensions({clientWidth: 600, clientHeight: 400}),
                };
            }"""
        )
        width, height = sizes["wide"]
        self.assertLessEqual(width * height, sizes["budget"] * 1.01)
        self.assertAlmostEqual(width / height, 2.0, places=2)
        self.assertEqual(sizes["small"], [1200, 800])

    def test_a_burst_of_pointer_events_draws_once_per_display_frame(self):
        draws = self.page.evaluate(
            """async () => {
                const {loadRuntime} = await import("./widget.js");
                const runtime = await loadRuntime(window.__molgfx.model);
                const render = runtime.Renderer.prototype.renderCamera;
                let count = 0;
                runtime.Renderer.prototype.renderCamera = function (...args) {
                    count += 1;
                    return render.apply(this, args);
                };
                const canvas = document.querySelector(".molgfx-canvas");
                const at = (x) => ({clientX: x, clientY: 50, pointerId: 1, isPrimary: true, bubbles: true});
                canvas.dispatchEvent(new PointerEvent("pointerdown", at(10)));
                for (let x = 11; x < 60; x += 1) canvas.dispatchEvent(new PointerEvent("pointermove", at(x)));
                await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
                canvas.dispatchEvent(new PointerEvent("pointerup", at(60)));
                runtime.Renderer.prototype.renderCamera = render;
                return count;
            }"""
        )
        self.assertGreaterEqual(draws, 1)
        self.assertLessEqual(draws, 2)
    def test_hover_pick_requests_are_coalesced_and_bounded(self):
        result = self.page.evaluate(
            """async () => {
                const {loadRuntime} = await import("./widget.js");
                const runtime = await loadRuntime(window.__molgfx.model);
                let active = 0;
                let maximum = 0;
                let calls = 0;
                const pending = [];
                runtime.Renderer.prototype.pick = function (x, y) {
                    calls += 1;
                    active += 1;
                    maximum = Math.max(maximum, active);
                    return new Promise((resolve) => pending.push(() => {
                        active -= 1;
                        resolve(JSON.stringify({x, y}));
                    }));
                };
                const canvas = document.querySelector(".molgfx-canvas");
                const move = (x) => canvas.dispatchEvent(new PointerEvent("pointermove", {
                    clientX: x, clientY: 50, pointerId: 1, bubbles: true,
                }));
                for (let x = 10; x < 110; x += 1) move(x);
                await new Promise((resolve) => requestAnimationFrame(resolve));
                const beforeRelease = [calls, maximum, pending.length];
                pending.shift()();
                await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
                while (pending.length) pending.shift()();
                return {beforeRelease, calls, maximum};
            }"""
        )
        self.assertLessEqual(result["maximum"], 1)
        self.assertLessEqual(result["beforeRelease"][0], 1)
        self.assertGreaterEqual(result["calls"], 2)

    def test_hover_does_not_save_changes_to_the_kernel(self):
        saved = self.page.evaluate(
            """async () => {
                const {loadRuntime} = await import("./widget.js");
                const runtime = await loadRuntime(window.__molgfx.model);
                runtime.Renderer.prototype.pick = () => Promise.resolve(JSON.stringify({hover: true}));
                const before = window.__molgfx.saved.length;
                document.querySelector(".molgfx-canvas").dispatchEvent(new PointerEvent("pointermove", {
                    clientX: 30, clientY: 50, pointerId: 1, bubbles: true,
                }));
                await new Promise((resolve) => setTimeout(resolve, 50));
                return window.__molgfx.saved.length - before;
            }"""
        )
        self.assertEqual(saved, 0)

    def test_pointerleave_discards_a_stale_hover_pick_result(self):
        result = self.page.evaluate(
            """async () => {
                const {loadRuntime} = await import("./widget.js");
                const runtime = await loadRuntime(window.__molgfx.model);
                let resolvePick;
                runtime.Renderer.prototype.pick = () => new Promise((resolve) => {
                    resolvePick = resolve;
                });
                const canvas = document.querySelector(".molgfx-canvas");
                canvas.dispatchEvent(new PointerEvent("pointermove", {
                    clientX: 30, clientY: 50, pointerId: 1, bubbles: true,
                }));
                await new Promise((resolve) => requestAnimationFrame(resolve));
                canvas.dispatchEvent(new PointerEvent("pointerleave", {bubbles: true}));
                resolvePick(JSON.stringify({stale: true}));
                await new Promise((resolve) => setTimeout(resolve, 25));
                return [window.__molgfx.values.pick, window.__molgfx.values.selection];
            }"""
        )
        self.assertEqual(result, [{}, ""])
    def test_a_pick_returns_semantic_provenance_not_a_gpu_token(self):
        before = self.page.evaluate("window.__molgfx.saved.length")
        self.page.evaluate("""() => { const canvas = document.querySelector('.molgfx-canvas'); const b = canvas.getBoundingClientRect(); canvas.dispatchEvent(new MouseEvent('click', {clientX: b.left + b.width / 2, clientY: b.top + b.height / 2, bubbles: true})); }""")
        self.page.wait_for_function("(before) => window.__molgfx.saved.length > before", arg=before)
        pick = self.page.evaluate("window.__molgfx.values.pick")
        self.assertEqual(pick["kind"], "atom")
        self.assertEqual(pick["dataset"], 1)
        self.assertIsInstance(pick["chunk"], int)
        self.assertIsInstance(pick["row"], int)
        self.assertIsNone(pick["volume_label"])

    def test_resize_reconfigures_the_canvas_in_device_pixels(self):
        sizes = self.page.evaluate("""async () => { const canvas = document.querySelector('.molgfx-canvas'); const before = [canvas.width, canvas.height]; document.getElementById('root').style.width = '320px'; await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))); return {before, after: [canvas.width, canvas.height], css: [canvas.clientWidth, canvas.clientHeight]}; }""")
        self.assertNotEqual(sizes["before"], sizes["after"])
        self.assertEqual(sizes["after"], sizes["css"])
    def test_resize_tracks_device_pixel_ratio_without_gpu_measurements(self):
        sizes = self.page.evaluate("""async () => {
            const canvas = document.querySelector('.molgfx-canvas');
            Object.defineProperty(window, 'devicePixelRatio', {configurable: true, value: 2});
            document.getElementById('root').style.width = '280px';
            await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
            return {canvas: [canvas.width, canvas.height], css: [canvas.clientWidth, canvas.clientHeight]};
        }""")
        self.assertEqual(sizes["canvas"], [2 * value for value in sizes["css"]])

    def test_camera_publication_is_accepted_as_finite_vec3_state(self):
        camera = self.page.evaluate("""async () => { const canvas = document.querySelector('.molgfx-canvas'); canvas.dispatchEvent(new WheelEvent('wheel', {deltaY: 20, bubbles: true, cancelable: true})); await new Promise((resolve) => setTimeout(resolve, 250)); return window.__molgfx.values.camera; }""")
        for name in ("position", "target", "up"):
            self.assertEqual(len(camera[name]), 3)
            self.assertTrue(all(isinstance(value, (int, float)) and value == value for value in camera[name]))

    def test_a_settled_scene_does_not_schedule_more_browser_frames(self):
        self.runtime()
        frames = self.page.evaluate(
            """async () => {
                const {loadRuntime} = await import("./widget.js");
                const runtime = await loadRuntime(window.__molgfx.model);
                const render = runtime.Renderer.prototype.renderCamera;
                await new Promise((resolve) => setTimeout(resolve, 500));
                let count = 0;
                runtime.Renderer.prototype.renderCamera = function (...args) {
                    count += 1;
                    return render.apply(this, args);
                };
                await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
                runtime.Renderer.prototype.renderCamera = render;
                return count;
            }"""
        )
        self.assertEqual(frames, 0)

    def test_the_kernel_hears_the_camera_once_after_a_wheel_burst(self):
        saved = self.page.evaluate(
            """async () => {
                const before = window.__molgfx.saved.length;
                const canvas = document.querySelector(".molgfx-canvas");
                for (let step = 0; step < 20; step += 1) {
                    canvas.dispatchEvent(new WheelEvent("wheel", {deltaY: 20, bubbles: true, cancelable: true}));
                }
                const during = window.__molgfx.saved.length - before;
                await new Promise((resolve) => setTimeout(resolve, 400));
                return [during, window.__molgfx.saved.length - before];
            }"""
        )
        self.assertEqual(saved, [0, 1])

    def test_the_same_grammar_runs_in_the_page(self):
        answer = self.page.evaluate(
            """async () => {
                const {loadRuntime} = await import("./widget.js");
                const runtime = await loadRuntime(window.__molgfx.model);
                const values = window.__molgfx.values;
                const scene = new runtime.Scene(values.scene_spec);
                scene.bindStructure(BigInt(values.structure_ids[0]), new Uint8Array(values.structure_payloads[0].buffer), values.structure_names[0]);
                scene.resolve();
                const session = new runtime.Session(scene);
                const good = JSON.parse(session.execute(scene, "select a, all; show cartoon, $a"));
                const bad = JSON.parse(session.execute(scene, "shwo cartoon, all"));
                return {good, bad, revision: Number(scene.revision)};
            }"""
        )
        self.assertTrue(answer["good"]["ok"], answer)
        self.assertEqual(answer["good"]["patch"]["operations"][0]["op"], "add_representation")
        self.assertFalse(answer["bad"]["ok"])
        self.assertEqual(answer["bad"]["errors"][0]["suggestion"], "show")

    def test_the_console_sends_commands_and_shows_answers(self):
        self.page.fill(".molgfx-input", "show cartoon, protein")
        self.page.press(".molgfx-input", "Enter")
        request = self.page.evaluate("window.__molgfx.values.command_request")
        self.assertEqual(request["type"], "execute")
        self.assertEqual(request["text"], "show cartoon, protein")
        self.page.evaluate(
            """(id) => {
                window.__molgfx.values.command_reply = {
                    type: "result", id, ok: false, text: "show cartoon, protein",
                    errors: [{message: "no protein here"}], rendered: "error: no protein here",
                };
                window.__molgfx.emit("change:command_reply");
            }""",
            request["id"],
        )
        failed = self.page.locator(".molgfx-log li.molgfx-failed")
        self.assertEqual(failed.count(), 1)
        self.assertIn("no protein here", failed.inner_text())
        self.assertIn("no protein here", self.page.inner_text(".molgfx-status"))

    def test_tab_asks_the_kernel_for_completions(self):
        self.page.fill(".molgfx-input", "show car")
        self.page.press(".molgfx-input", "Tab")
        request = self.page.evaluate("window.__molgfx.values.command_request")
        self.assertEqual(request["type"], "complete")
        self.assertEqual(request["cursor"], 8)
        self.page.evaluate(
            """(id) => {
                window.__molgfx.values.command_reply = {
                    type: "completions", id,
                    items: [{text: "cartoon", kind: "form", detail: ""}],
                };
                window.__molgfx.emit("change:command_reply");
            }""",
            request["id"],
        )
        self.assertEqual(self.page.input_value(".molgfx-input"), "show cartoon")


if __name__ == "__main__":
    unittest.main()
