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
from molgfx.viewer.viewer import _runtime

from test_viewer import PAGE, STATIC, _LocalPage, structure, sync_playwright


def _values(bench):
    """The synchronized state a frontend receives for ``bench``."""
    sources = bench.scene._browser_sources()
    code, data, key = _runtime()
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
        "command_reply": {},
        "_runtime_js": code,
        "_runtime_wasm": base64.b64encode(data).decode("ascii"),
        "_runtime_key": key,
    }


@unittest.skipUnless(_runtime()[2], "the browser runtime is not built")
class RuntimeTransportTests(unittest.TestCase):
    def test_the_runtime_travels_compressed_and_inflates_to_the_packaged_binary(self):
        _code, data, _key = _runtime()
        packaged = (STATIC / "molgfx_wasm_bg.wasm").read_bytes()
        self.assertEqual(gzip.decompress(data), packaged)
        self.assertLess(len(data), len(packaged) // 2)


@unittest.skipUnless(sync_playwright is not None, "playwright is not installed")
@unittest.skipUnless(_runtime()[2], "the browser runtime is not built")
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
