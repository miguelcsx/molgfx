#!/usr/bin/env python3
"""Compare disposable volume-file formats across browser engines and molgfx.

The fixtures are intentionally small and synthetic.  A successful load proves
that the engine accepted the format and reached a rendered volume component; it
does not claim numerical or image equivalence between the engines.
"""

from __future__ import annotations

import argparse
import functools
import json
import os
import shutil
import struct
import subprocess
import tempfile
import threading
import time
from dataclasses import dataclass
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from graphics_engine_volume_differential import (
    browser_json,
    browser_result,
    close_browser,
    image_summary,
    sha256,
    wait_audit,
    write_mrc,
)


@dataclass(frozen=True)
class FormatCase:
    name: str
    extension: str
    molstar_format: str
    binary: bool
    filename: str | None = None
    ngl_extension: str | None = None


CASES = (
    FormatCase("mrc", "mrc", "ccp4", True),
    FormatCase("map", "map", "ccp4", True),
    FormatCase("ccp4", "ccp4", "ccp4", True),
    FormatCase("cube", "cube", "cube", False),
    FormatCase("dx", "dx", "dx", False),
    FormatCase("dsn6", "dsn6", "dsn6", True),
    FormatCase("brix", "brix", "dsn6", True),
    FormatCase("mrc-mode1", "mrc", "ccp4", True, "fixture-mode1.mrc", "mrc"),
    FormatCase("mrc-mode0", "mrc", "ccp4", True, "fixture-mode0.mrc", "mrc"),
)


NGL_PREFIX = r"""<!doctype html><meta charset="utf-8">
<style>html,body{width:100%;height:100%;margin:0;background:#fff}#viewport{width:256px;height:256px}</style>
<div id="viewport"></div>
<script src="/node_modules/three/build/three.js"></script><script>window.three=window.THREE;</script>
<script src="/node_modules/chroma-js/chroma.min.js"></script>
<script src="/node_modules/signals/dist/signals.js"></script><script>window.signalsWrapper=window.signals;</script>
<script src="/node_modules/sprintf-js/dist/sprintf.min.js"></script><script>window.sprintfJs={sprintf:window.sprintf,vsprintf:window.vsprintf};</script>
<script src="/node_modules/ngl/dist/ngl.umd.js"></script>
"""
MOLSTAR_PREFIX = r"""<!doctype html><meta charset="utf-8">
<link rel="stylesheet" href="/node_modules/molstar/build/viewer/molstar.css">
<style>html,body{width:100%;height:100%;margin:0;background:#fff}#viewport{width:256px;height:256px;position:relative}</style>
<div id="viewport"></div><script src="/node_modules/molstar/build/viewer/molstar.js"></script>
"""


def javascript_string(value: str) -> str:
    return json.dumps(value).replace("<", "\\u003c")


def cube_values(size: int = 16) -> list[float]:
    center = (size - 1) * 0.5
    values: list[float] = []
    for z in range(size):
        for y in range(size):
            for x in range(size):
                distance = sum(
                    ((coordinate - center) / (size * 0.20)) ** 2
                    for coordinate in (x, y, z)
                )
                values.append(2.0 ** (-distance))
    return values


def write_cube(path: Path, size: int = 16) -> None:
    values = cube_values(size)
    lines = [
        "MOLGFX VOLUME FORMAT PROBE",
        "synthetic scalar field",
        f"1 0.0 0.0 0.0",
        f"{size} 0.5 0.0 0.0",
        f"{size} 0.0 0.5 0.0",
        f"{size} 0.0 0.0 0.5",
        "6 0.0 0.0 0.0 0.0",
    ]
    for index in range(0, len(values), 6):
        lines.append(" ".join(f"{value:.6e}" for value in values[index : index + 6]))
    path.write_text("\n".join(lines) + "\n", encoding="ascii")


def write_mrc_integer(path: Path, mode: int, size: int = 16) -> None:
    values = [round(value * (100 if mode == 0 else 1000)) for value in cube_values(size)]
    minimum, maximum = min(values), max(values)
    mean = sum(values) / len(values)
    header = bytearray(1024)
    struct.pack_into("<3i", header, 0, size, size, size)
    struct.pack_into("<i", header, 12, mode)
    struct.pack_into("<3i", header, 28, size, size, size)
    struct.pack_into("<3f", header, 40, float(size), float(size), float(size))
    struct.pack_into("<3f", header, 52, 90.0, 90.0, 90.0)
    struct.pack_into("<3i", header, 64, 1, 2, 3)
    struct.pack_into("<3f", header, 76, minimum, maximum, mean)
    header[208:212] = b"MAP "
    header[212:216] = b"DD\x00\x00"
    if mode == 0:
        encoded = struct.pack(f"<{len(values)}b", *values)
    else:
        encoded = struct.pack(f"<{len(values)}h", *values)
    path.write_bytes(header + encoded)


def write_dx(path: Path, size: int = 16) -> None:
    values = cube_values(size)
    lines = [
        f"object 1 class gridpositions counts {size} {size} {size}",
        "origin 0.0 0.0 0.0",
        "delta 0.5 0.0 0.0",
        "delta 0.0 0.5 0.0",
        "delta 0.0 0.0 0.5",
        f"object 2 class gridconnections counts {size} {size} {size}",
        f"object 3 class array type double rank 0 items {len(values)} data follows",
    ]
    for index in range(0, len(values), 3):
        lines.append(" ".join(f"{value:.8e}" for value in values[index : index + 3]))
    lines.extend(
        [
            'attribute "dep" string "positions"',
            'object "molgfx-volume" class field',
            'component "positions" value 1',
            'component "connections" value 2',
            'component "data" value 3',
        ]
    )
    path.write_text("\n".join(lines) + "\n", encoding="ascii")


def dsn6_samples(size: int = 16) -> bytes:
    values = cube_values(size)
    encoded = bytearray()
    blocks = (size + 7) // 8
    for zz in range(blocks):
        for yy in range(blocks):
            for xx in range(blocks):
                for k in range(8):
                    for j in range(8):
                        for i in range(8):
                            x, y, z = xx * 8 + i, yy * 8 + j, zz * 8 + k
                            value = values[z * size * size + y * size + x] if x < size and y < size and z < size else 0.0
                            encoded.append(max(0, min(255, round(value * 255))))
    return bytes(encoded)


def write_dsn6(path: Path, size: int = 16) -> None:
    header = bytearray(512)
    fields = [0, 0, 0, size, size, size, 1, 1, 1, 1000, 1000, 1000, 9000, 9000, 9000, 100, 0, 100, 100]
    struct.pack_into("<19h", header, 0, *fields)
    path.write_bytes(header + dsn6_samples(size))


def write_brix(path: Path, size: int = 16) -> None:
    header = [" "] * 512

    def put(offset: int, width: int, value: str) -> None:
        header[offset : offset + width] = list(value[:width].ljust(width))

    put(0, 3, ":-)")
    for offset, value in ((10, 0), (15, 0), (20, 0), (32, size), (38, size), (42, size), (52, 1), (58, 1), (62, 1)):
        put(offset, 5, f"{value:5d}")
    for offset, value in ((73, 10.0), (83, 10.0), (93, 10.0), (103, 90.0), (113, 90.0), (123, 90.0)):
        put(offset, 10, f"{value:10.3f}")
    put(138, 12, f"{100.0:12.3f}")
    put(155, 8, f"{0:8d}")
    put(170, 12, f"{1.0:12.3f}")
    path.write_bytes("".join(header).encode("ascii") + dsn6_samples(size))


def write_fixtures(directory: Path) -> dict[str, dict[str, Any]]:
    mrc = directory / "fixture.mrc"
    write_mrc(mrc, (16, 16, 16))
    records: dict[str, dict[str, Any]] = {}
    for case in CASES:
        path = directory / (case.filename or f"fixture.{case.extension}")
        if case.name == "mrc":
            pass
        elif case.name in {"map", "ccp4"}:
            shutil.copyfile(mrc, path)
        elif case.name == "cube":
            write_cube(path)
        elif case.name == "dx":
            write_dx(path)
        elif case.name == "dsn6":
            write_dsn6(path)
        elif case.name == "brix":
            write_brix(path)
        elif case.name == "mrc-mode1":
            write_mrc_integer(path, 1)
        elif case.name == "mrc-mode0":
            write_mrc_integer(path, 0)
        records[case.name] = {
            "path": path.name,
            "format": case.molstar_format,
            "binary": case.binary,
            "bytes": path.stat().st_size,
            "sha256": sha256(path),
        }
    return records


def html_for(engine: str, case: FormatCase) -> str:
    filename = case.filename or f"fixture.{case.extension}"
    if engine == "ngl":
        body = f"""
<script>
window.audit={{status:'loading',error:null,dataUrl:null}};
addEventListener('load',async()=>{{try{{
 const stage=new NGL.Stage('viewport',{{backgroundColor:'white'}});
 const component=await stage.loadFile('/{filename}',{{ext:{javascript_string(case.ngl_extension or case.extension)}}});
 if(component.type!=='volume') throw Error('NGL component type was '+component.type);
 component.addRepresentation('surface',{{isolevel:{{type:'value',value:0.2}}}});
 component.autoView(); stage.handleResize(); stage.viewer.requestRender();
 await new Promise(resolve=>setTimeout(resolve,900));
 const canvas=document.querySelector('#viewport canvas');
 if(!canvas) throw Error('NGL canvas was not created');
 const capture=document.createElement('canvas');capture.width=256;capture.height=256;
 capture.getContext('2d').drawImage(canvas,0,0,256,256);
 window.audit={{status:'passed',type:component.type,width:canvas.width,height:canvas.height,
  dataUrl:capture.toDataURL('image/png')}};
}}catch(error){{window.audit={{status:'failed',error:String(error)}};}}}});
</script>"""
        return NGL_PREFIX + body
    body = f"""
<script>
window.audit={{status:'loading',error:null,dataUrl:null}};
addEventListener('load',async()=>{{try{{
 const viewer=await molstar.Viewer.create('viewport',{{layoutShowControls:false,
  viewportShowExpand:false,viewportShowSelectionMode:false,viewportBackgroundColor:'#ffffff'}});
 await viewer.loadVolumeFromUrl({{url:'/{filename}',format:{javascript_string(case.molstar_format)},
  isBinary:{str(case.binary).lower()}}},[]);
 const volume=viewer.plugin.managers.volume.hierarchy.current.volumes[0];
 if(!volume) throw Error('Mol* volume was not created');
 await viewer.plugin.managers.volume.hierarchy.addRepresentation(volume,'isosurface');
 viewer.plugin.managers.camera.reset({{durationMs:0}});
 await new Promise(resolve=>setTimeout(resolve,1200));
 const canvas=document.querySelector('#viewport canvas');
 if(!canvas) throw Error('Mol* canvas was not created');
 const capture=document.createElement('canvas');capture.width=256;capture.height=256;
 capture.getContext('2d').drawImage(canvas,0,0,256,256);
 window.audit={{status:'passed',type:'volume',width:canvas.width,height:canvas.height,
  dataUrl:capture.toDataURL('image/png')}};
}}catch(error){{window.audit={{status:'failed',error:String(error)}};}}}});
</script>"""
    return MOLSTAR_PREFIX + body


def capture_data_url(browser: str, session: str, output: Path) -> dict[str, Any]:
    data_url = browser_result(browser, session, "window.audit.dataUrl")
    prefix, separator, encoded = data_url.partition(",")
    if not separator or not prefix.startswith("data:image/png;base64"):
        raise RuntimeError("browser did not return a PNG data URL")
    import base64

    output.write_bytes(base64.b64decode(encoded, validate=True))
    return image_summary(output)


def run_browser_cases(
    browser: str, node_root: Path, directory: Path
) -> dict[str, list[dict[str, Any]]]:
    session = f"molgfx-volume-formats-{os.getpid()}"
    results: dict[str, list[dict[str, Any]]] = {"ngl": [], "molstar": []}
    webroot = directory / "web"
    webroot.mkdir()
    (webroot / "node_modules").symlink_to(node_root / "node_modules", target_is_directory=True)
    for case in CASES:
        shutil.copyfile(
            directory / (case.filename or f"fixture.{case.extension}"),
            webroot / (case.filename or f"fixture.{case.extension}"),
        )

    class QuietHandler(SimpleHTTPRequestHandler):
        def log_message(self, _format: str, *_args: Any) -> None:
            return

    server = ThreadingHTTPServer(
        ("127.0.0.1", 0), functools.partial(QuietHandler, directory=str(webroot))
    )
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        for engine in results:
            for case in CASES:
                page = webroot / f"{engine}-{case.name}.html"
                page.write_text(html_for(engine, case), encoding="utf-8")
                record: dict[str, Any] = {"format": case.name, "status": "failed"}
                try:
                    browser_json(
                        browser,
                        session,
                        ["open", f"http://127.0.0.1:{server.server_port}/{page.name}"],
                    )
                    audit = wait_audit(browser, session)
                    record.update(audit)
                    if audit.get("status") == "passed":
                        record["image"] = capture_data_url(
                            browser, session, directory / f"{engine}-{case.name}.png"
                        )
                except Exception as error:
                    record["error"] = f"{type(error).__name__}: {error}"[:500]
                results[engine].append(record)
    finally:
        close_browser(browser, session)
        server.shutdown()
        server.server_close()
    return results


def run_molgfx(executable: Path, directory: Path) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for case in CASES:
        output = directory / f"molgfx-{case.name}.png"
        completed = subprocess.run(
            [
                str(executable),
                str(output),
                "isosurface",
                "probe",
                str(directory / (case.filename or f"fixture.{case.extension}")),
            ],
            capture_output=True,
            text=True,
            check=False,
            env={**os.environ, "WGPU_BACKEND": "metal"},
        )
        record: dict[str, Any] = {
            "format": case.name,
            "returncode": completed.returncode,
            "status": "passed" if completed.returncode == 0 else "failed",
            "stdout": completed.stdout[-500:],
            "stderr": completed.stderr[-500:],
        }
        if completed.returncode == 0 and output.is_file():
            record["image"] = image_summary(output)
        results.append(record)
    return results


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser-use", default="browser-use")
    parser.add_argument("--node-root", type=Path, required=True)
    parser.add_argument("--molgfx-example", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    with tempfile.TemporaryDirectory(prefix="molgfx-volume-formats-") as directory_name:
        directory = Path(directory_name)
        fixtures = write_fixtures(directory)
        result: dict[str, Any] = {
            "schema": 1,
            "scope": "disposable volume-format load/render diagnostics; not numerical or image equivalence",
            "references": {
                "ngl": "https://nglviewer.org/ngl/api/manual/usage/volume-representations.html",
                "molstar": "https://molstar.org/docs/plugin/file-formats/",
            },
            "fixtures": fixtures,
        }
        result["browser"] = run_browser_cases(
            args.browser_use, args.node_root.resolve(), directory
        )
        if args.molgfx_example:
            result["molgfx"] = run_molgfx(args.molgfx_example.resolve(), directory)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
