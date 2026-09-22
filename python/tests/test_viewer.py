"""Browser tests for the committed anywidget against the packaged WASM runtime.

Two layers live here. The transport layer needs no browser: it drives the
committed ``Viewer`` with a real ``molgfx.Scene`` and asserts what crosses the
widget's synchronized trait surface. The page layer drives the committed
``widget.js`` entry point — the same ``render({model, el})`` contract anywidget
calls — in a real browser, so patch handling, interaction and canvas behavior are
exercised through the shipped module rather than a reimplementation of it.

The page layer is skipped when playwright or its chromium build is unavailable,
so the ordinary ``unittest discover`` job stays green without a browser.
"""

import base64
import json
from pathlib import Path
import threading
import unittest
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer

import molframe
import molgfx
from molgfx.viewer import Viewer

try:  # pragma: no cover - present only in the browser job
    from playwright.sync_api import sync_playwright

    PLAYWRIGHT_IMPORT_ERROR = ""
except ImportError as error:  # pragma: no cover - the default job takes this path
    sync_playwright = None
    PLAYWRIGHT_IMPORT_ERROR = str(error)


STATIC = Path(__file__).resolve().parents[1] / "molgfx" / "viewer" / "static"

MMCIF = b"""data_one
_entry.id one
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
ATOM 1 N N ALA A 1 1 11.104 13.207 9.274 1.00 20.00
ATOM 2 C CA ALA A 1 1 12.560 13.318 9.111 1.00 20.00
ATOM 3 C C ALA A 1 1 13.100 14.600 9.900 1.00 20.00
ATOM 4 O O ALA A 1 1 14.200 14.700 10.100 1.00 20.00
"""


def structure():
    """The smallest renderable structure: four atoms of one residue."""
    return molframe.read(MMCIF, name="one.cif")


def second_structure():
    """A distinct second structure, so its payload cannot alias the first."""
    return molframe.read(
        MMCIF.replace(b"data_one", b"data_two")
        + b"ATOM 5 CB CB ALA A 1 1 15.300 15.400 10.800 1.00 20.00\n",
        name="two.cif",
    )


def opacity_patch(scene, representation, opacity=0.25):
    """One semantic edit of one live representation."""
    return json.dumps(
        {
            "base_revision": scene.revision,
            "operations": [
                {
                    "op": "set_opacity",
                    "id": int(str(representation).split("(")[-1].rstrip(")")),
                    "opacity": opacity,
                }
            ],
        }
    )


class CountingScene:
    """A real scene that records how often the transport is entered.

    Subscriptions are relayed to the real scene, so publications observed here
    are the ones the Rust scene actually emitted.
    """

    def __init__(self, scene):
        self._scene = scene
        self.source_calls = 0

    def _browser_sources(self):
        self.source_calls += 1
        return self._scene._browser_sources()

    def _subscribe(self, subscriber):
        def relay():
            return subscriber()

        self._scene._subscribe(relay)

    def apply(self, patch):
        self._scene.apply(patch)

    def to_json(self):
        return self._scene.to_json()


def viewer_with_representation():
    """A viewer over one structure with one representation."""
    scene = CountingScene(molgfx.Scene(structure()))
    representation = scene._scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
    return scene, Viewer(scene), representation


class ViewerTransportTests(unittest.TestCase):
    """What the widget synchronizes, driven without a browser."""

    def test_the_initial_source_transfer_happens_exactly_once(self):
        scene = CountingScene(molgfx.Scene(structure()))
        representation = scene._scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))

        viewer = Viewer(scene)
        self.assertEqual(scene.source_calls, 1)
        self.assertEqual(len(viewer.structure_payloads), 1)
        payloads = [bytes(payload) for payload in viewer.structure_payloads]

        # A later edit must reuse the materialized payload: the page already
        # holds it, and re-encoding would re-serialize every atom.
        scene._scene.set_opacity(representation, 0.5)
        self.assertEqual(viewer.patch_sequence, 1)
        self.assertEqual(scene.source_calls, 1)
        self.assertEqual(
            [bytes(payload) for payload in viewer.structure_payloads], payloads
        )

    def test_a_direct_scene_mutation_reaches_the_page_as_one_patch(self):
        scene, viewer, representation = viewer_with_representation()

        scene._scene.set_opacity(representation, 0.25)

        self.assertEqual(viewer.patch_sequence, 1)
        patch = json.loads(viewer.scene_patch)
        self.assertEqual(len(patch["operations"]), 1)
        self.assertEqual(patch["operations"][0]["op"], "set_opacity")
        self.assertEqual(patch["operations"][0]["opacity"], 0.25)

    def test_an_announced_structure_materializes_its_payload_once(self):
        scene, viewer, representation = viewer_with_representation()
        spec_before = viewer.scene_spec
        self.assertEqual(scene.source_calls, 1)

        scene._scene._add_structure(second_structure())

        self.assertEqual(viewer.structure_ids, [1, 2])
        self.assertEqual(len(viewer.structure_payloads), 2)
        self.assertNotEqual(viewer.structure_payloads[0], viewer.structure_payloads[1])
        # The page rebuilds from the spec, so the announcement is not also
        # forwarded as a patch applied to an already-resolved scene.
        self.assertEqual(viewer.patch_sequence, 0)
        self.assertNotEqual(viewer.scene_spec, spec_before)
        self.assertIn('"2"', viewer.scene_spec)
        payloads = [bytes(payload) for payload in viewer.structure_payloads]
        self.assertEqual(scene.source_calls, 2)

        # A later edit is still one ordinary patch and re-reads no payload.
        scene._scene.set_opacity(representation, 0.5)
        self.assertEqual(viewer.patch_sequence, 1)
        self.assertEqual(scene.source_calls, 2)
        self.assertEqual(
            [bytes(payload) for payload in viewer.structure_payloads], payloads
        )

    def test_a_second_structure_renders_and_resolves_on_the_rust_scene(self):
        scene = CountingScene(molgfx.Scene(structure()))
        scene._scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))

        identity = scene._scene._add_structure(second_structure())
        self.assertEqual(int(str(identity).split("(")[-1].rstrip(")")), 2)

        spec = json.loads(scene.to_json())
        self.assertEqual(sorted(spec["structures"]), ["1", "2"])
        self.assertEqual(len(scene._browser_sources()), 2)

    def test_no_frame_is_transported_as_png_or_base64(self):
        scene, viewer, representation = viewer_with_representation()
        scene._scene.set_opacity(representation, 0.25)

        for value in [
            viewer.scene_spec,
            viewer.scene_patch,
            viewer.selection,
            viewer.error,
            json.dumps(viewer.camera),
            json.dumps(viewer.pick),
        ]:
            self.assertNotIn("data:image", value)
            self.assertNotIn("iVBOR", value)
        for payload in viewer.structure_payloads:
            self.assertFalse(bytes(payload).startswith(b"\x89PNG"))

        widget = (STATIC / "widget.js").read_text()
        for forbidden in ["toDataURL", "toBlob", "base64", "image/png"]:
            self.assertNotIn(forbidden, widget)


class _SilentHandler(SimpleHTTPRequestHandler):
    def log_message(self, *args):
        """Keep the browser job's output to test results only."""


class _LocalPage:
    """A static server over the committed widget assets."""

    def __init__(self, directory):
        self.directory = str(directory)
        self.port = 0
        self._server = None
        self._thread = None

    def __enter__(self):
        directory = self.directory

        class Handler(_SilentHandler):
            def __init__(self, *args, **kwargs):
                super().__init__(*args, directory=directory, **kwargs)

        self._server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self._server.daemon_threads = True
        self.port = self._server.server_address[1]
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)
        self._thread.start()
        return self

    def __exit__(self, *exc):
        self._server.shutdown()
        self._thread.join(timeout=5)
        self._server.server_close()
        return False


PAGE = """<!doctype html>
<html><body style="margin:0">
<div id="root" style="width:320px;height:240px"></div>
<script type="module">
import {render} from "./widget.js";

const listeners = new Map();
const saved = [];
window.__molgfx = {
  values: __VALUES__,
  saved,
  setupError: "",
  model: null,
  emit: (event) => { const callback = listeners.get(event); if (callback) callback(); },
  bytes: (encoded) => encoded.map((text) => {
    const raw = atob(text);
    const bytes = new Uint8Array(raw.length);
    for (let index = 0; index < raw.length; index += 1) bytes[index] = raw.charCodeAt(index);
    return new DataView(bytes.buffer);
  }),
};
const snapshot = () => JSON.stringify(window.__molgfx.values, (key, value) =>
  value instanceof DataView ? Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength)) : value);
// anywidget delivers byte traits as base64 text; materialize them before render.
window.__molgfx.values.structure_payloads = window.__molgfx.bytes(window.__molgfx.values.structure_payloads);
window.__molgfx.model = {
  get: (name) => window.__molgfx.values[name],
  set: (name, value) => { window.__molgfx.values[name] = value; },
  save_changes: () => { saved.push(snapshot()); },
  on: (event, callback) => { listeners.set(event, callback); },
  off: (event) => { listeners.delete(event); },
};
try {
  window.__molgfx.cleanup = await render({
    model: window.__molgfx.model,
    el: document.getElementById("root"),
  });
} catch (error) {
  window.__molgfx.setupError = error instanceof Error ? error.message : String(error);
}
</script></body></html>
"""


@unittest.skipUnless(sync_playwright is not None, "playwright is not installed")
class ViewerPageTests(unittest.TestCase):
    """The committed widget module, driven in a real browser."""

    @classmethod
    def setUpClass(cls):
        scene = molgfx.Scene(structure())
        scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
        sources = scene._browser_sources()
        values = {
            "scene_spec": scene.to_json(),
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
        }
        (STATIC / "_viewer_test_page.html").write_text(
            PAGE.replace("__VALUES__", json.dumps(values))
        )
        cls.revision = scene.revision

        cls._playwright = sync_playwright().start()
        cls._local = _LocalPage(STATIC).__enter__()
        cls._browser = cls._playwright.chromium.launch(
            args=["--enable-unsafe-webgpu", "--use-angle=swiftshader"]
        )
        cls.page = cls._browser.new_page()
        cls.page.goto(f"http://127.0.0.1:{cls._local.port}/_viewer_test_page.html")
        cls.page.wait_for_function("window.__molgfx.cleanup !== undefined")

    @classmethod
    def tearDownClass(cls):
        cls.page.evaluate("window.__molgfx.cleanup && window.__molgfx.cleanup()")
        cls._browser.close()
        cls._playwright.stop()
        cls._local.__exit__(None, None, None)
        (STATIC / "_viewer_test_page.html").unlink(missing_ok=True)

    def runtime(self):
        """The page's live widget state, skipping when the GPU runtime is absent."""
        setup_error = self.page.evaluate("window.__molgfx.setupError")
        if setup_error:
            self.skipTest(f"the browser runtime is unavailable: {setup_error}")
        return self.page.evaluate("window.__molgfx.values")

    def test_the_page_binds_every_transported_structure_and_draws(self):
        values = self.runtime()
        self.assertEqual(values["error"], "")
        self.assertEqual(len(values["structure_ids"]), 1)
        self.assertIsNotNone(self.page.evaluate("document.querySelector('canvas')"))

    def test_one_patch_reaches_the_page_and_applies(self):
        values = self.runtime()
        self.page.evaluate(
            """(patch) => {
                window.__molgfx.values.scene_patch = patch;
                window.__molgfx.values.patch_sequence += 1;
                window.__molgfx.emit("change:patch_sequence");
            }""",
            opacity_patch(_Scene(), 0.4),
        )
        self.page.wait_for_timeout(300)
        values = self.page.evaluate("window.__molgfx.values")
        self.assertEqual(values["error"], "")
        self.assertEqual(values["patch_sequence"], 1)
        self.assertIn("set_opacity", values["scene_patch"])

    def test_a_rebuilt_scene_transports_each_payload_once(self):
        self.runtime()
        before = self.page.evaluate("window.__molgfx.values.structure_payloads.length")
        self.page.evaluate(
            """(columns) => {
                window.__molgfx.values.structure_ids = columns.ids;
                window.__molgfx.values.structure_names = columns.names;
                window.__molgfx.values.structure_payloads = window.__molgfx.bytes(columns.payloads);
                window.__molgfx.values.scene_spec = columns.spec;
                window.__molgfx.emit("change:scene_spec");
            }""",
            _two_structure_columns(),
        )
        self.page.wait_for_timeout(400)
        values = self.page.evaluate("window.__molgfx.values")
        self.assertEqual(values["error"], "")
        self.assertEqual(before, 1)
        self.assertEqual(values["structure_payloads"].length, 2)

    def test_resize_reconfigures_the_canvas(self):
        self.runtime()
        before = self.page.evaluate("document.querySelector('canvas').width")
        self.page.set_viewport_size({"width": 900, "height": 600})
        self.page.wait_for_timeout(300)
        self.assertNotEqual(
            self.page.evaluate("document.querySelector('canvas').width"), before
        )

    def test_an_interaction_event_round_trips(self):
        self.runtime()
        box = self.page.locator("canvas").bounding_box()
        self.page.mouse.click(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
        self.page.wait_for_timeout(500)
        values = self.page.evaluate("window.__molgfx.values")
        self.assertEqual(values["error"], "")
        self.assertIn("selection", values)
        self.assertGreater(self.page.evaluate("window.__molgfx.saved.length"), 0)
        self.assertIsInstance(values["pick"], dict)

    def test_no_frame_is_transported_as_png_or_base64(self):
        self.runtime()
        self.page.evaluate(
            """() => {
                window.__molgfx.values.selection = "1:A:1";
                window.__molgfx.model.save_changes();
            }"""
        )
        snapshots = self.page.evaluate("window.__molgfx.saved")
        self.assertGreater(len(snapshots), 0)
        for snapshot in snapshots:
            self.assertNotIn("data:image", snapshot)
            self.assertNotIn("iVBOR", snapshot)


def _Scene():
    """A scene identical to the page's, for building patches against it."""
    scene = molgfx.Scene(structure())
    scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
    return scene


def _two_structure_columns():
    """Transport columns and spec for a page holding two structures."""
    scene = molgfx.Scene(structure())
    scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
    scene._add_structure(second_structure())
    sources = scene._browser_sources()
    return {
        "ids": [source[0] for source in sources],
        "names": [source[1] for source in sources],
        "payloads": [
            base64.b64encode(bytes(source[2])).decode("ascii") for source in sources
        ],
        "spec": scene.to_json(),
    }
