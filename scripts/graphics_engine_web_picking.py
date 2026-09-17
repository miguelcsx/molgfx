#!/usr/bin/env python3
"""Probe screen-space molecular picking in disposable NGL and Mol* pages.

The probe asks each browser renderer to resolve a rendered canvas coordinate to
an atom-level structure entity.  The bundles, page, browser session and PNG
stay outside the repository; the JSON result is the only durable artifact.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import shutil
import tempfile
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import threading
from pathlib import Path
from typing import Any

from graphics_engine_cross_render import image_summary
from graphics_engine_web_differential import (
    MOLSTAR_PREFIX,
    NGL_PREFIX,
    browser_json,
    browser_result,
    close_browser,
    javascript_string,
    wait_audit,
)


def capture_pick_image(browser: str, session: str, output: Path) -> dict[str, Any]:
    """Decode the page's actual WebGL canvas capture into the disposable dir."""

    data_url = browser_result(browser, session, "window.audit.dataUrl")
    prefix, separator, encoded = data_url.partition(",")
    if not separator or not prefix.startswith("data:image/png;base64"):
        raise RuntimeError("browser did not return a PNG data URL")
    output.write_bytes(base64.b64decode(encoded, validate=True))
    return image_summary(output)


def page_html(engine: str, fixture: str) -> str:
    """Build one disposable page using the engine's public picking API."""

    fixture_literal = javascript_string(fixture)
    if engine == "ngl":
        body = f"""
<script>
window.audit={{status:'loading',error:null,dataUrl:null,picked:null}};
const fixture={fixture_literal};
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const capture=()=>{{const source=document.querySelector('#viewport canvas');
  const target=document.createElement('canvas');target.width=256;target.height=256;
  target.getContext('2d').drawImage(source,0,0,256,256);return target.toDataURL('image/png')}};
const findPick=(stage,width,height)=>{{
  for(let y=0;y<height;y+=4) for(let x=0;x<width;x+=4) {{
    const proxy=stage.pickingControls.pick(x,y);
    const atom=proxy&&(proxy.atom||proxy.closestBondAtom);
    if(atom) return {{x,y,type:proxy.type,atomIndex:atom.index,
      qualifiedName:atom.qualifiedName(),position:proxy.position ?
      [proxy.position.x,proxy.position.y,proxy.position.z] : null}};
  }}
  return null;
}};
addEventListener('load',async()=>{{try{{
  const stage=new NGL.Stage('viewport',{{backgroundColor:'white'}});
  const component=await stage.loadFile(new Blob([fixture],{{type:'text/plain'}}),{{ext:'cif'}});
  component.addRepresentation('ball+stick',{{}});component.autoView(0);
  stage.handleResize();stage.viewer.requestRender();await wait(800);
  const canvas=document.querySelector('#viewport canvas');
  if(!canvas) throw Error('NGL canvas was not created');
  const picked=findPick(stage,canvas.width,canvas.height);
  window.audit={{status:picked?'passed':'failed',error:picked?null:'no atom was picked',
    engine:'ngl',canvasWidth:canvas.width,canvasHeight:canvas.height,
    picked,dataUrl:capture()}};
}}catch(error){{window.audit={{status:'failed',error:String(error)}}}}}});
</script>"""
        return NGL_PREFIX + body

    body = f"""
<script>
window.audit={{status:'loading',error:null,dataUrl:null,picked:null}};
const fixture={fixture_literal};
const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const capture=()=>{{const source=document.querySelector('#viewport canvas');
  const target=document.createElement('canvas');target.width=256;target.height=256;
  target.getContext('2d').drawImage(source,0,0,256,256);return target.toDataURL('image/png')}};
const findPick=(plugin,width,height)=>{{
  let firstPick=null;
  let pickCount=0;let nonNullPickCount=0;
  const lociKinds={{}};
  for(let y=0;y<height;y+=4) for(let x=0;x<width;x+=4) {{
    const pick=plugin.canvas3d.identify(new Float32Array([x,y]));
    if(!pick) continue;
    pickCount+=1;
    if(pick.id&&pick.id.objectId!==undefined&&pick.id.objectId!==null) nonNullPickCount+=1;
    const hasId=pick.id&&pick.id.objectId!==undefined&&pick.id.objectId!==null;
    if(hasId&&!firstPick) firstPick={{x,y,id:pick.id,position:Array.from(pick.position)}};
    const lociResult=plugin.canvas3d.getLoci(pick.id);
    const loci=lociResult&&lociResult.loci;
    if(loci) lociKinds[loci.kind]=(lociKinds[loci.kind]||0)+1;
    if(!loci||loci.kind==='empty-loci'||!loci.elements||!loci.elements.length) continue;
    const element=loci.elements[0];
    const atomIndex=element.unit.elements[0];
    const atoms=element.unit.model.atomicHierarchy.atoms;
    return {{x,y,lociKind:loci.kind,lociElementCount:loci.elements.length,atomIndex,
      atomName:atoms.label_atom_id.value(atomIndex)}};
  }}
  return firstPick ? {{pickDebug:firstPick,pickCount,nonNullPickCount,lociKinds}} : null;
}};
addEventListener('load',async()=>{{try{{
  const viewer=await molstar.Viewer.create('viewport',{{layoutShowControls:false,
    viewportShowExpand:false,viewportShowSelectionMode:false,viewportBackgroundColor:'#ffffff'}});
  await viewer.loadStructureFromData(fixture,'mmcif');
  const plugin=viewer.plugin;
  const structure=plugin.managers.structure.hierarchy.current.structures[0];
  if(!structure) throw Error('Mol* structure was not created');
  const component=await plugin.builders.structure.tryCreateComponentStatic(structure.cell,'polymer')
    || await plugin.builders.structure.tryCreateComponentStatic(structure.cell,'all');
  if(!component) throw Error('Mol* component was not created');
  await plugin.builders.structure.representation.addRepresentation(component,
    {{type:'cartoon',color:'chain-id'}});
  plugin.managers.camera.reset({{durationMs:0}});plugin.canvas3d.commit(true);
  plugin.canvas3d.requestDraw();await wait(1200);
  const canvas=document.querySelector('#viewport canvas');
  if(!canvas) throw Error('Mol* canvas was not created');
  const bounds=canvas.getBoundingClientRect();
  const picked=findPick(plugin,Math.ceil(bounds.width),Math.ceil(bounds.height));
  const input=plugin.canvas3d.input;
  window.audit={{status:picked&&picked.lociKind?'passed':'failed',error:picked&&picked.lociKind?null:'no structure loci was picked',
    engine:'molstar',canvasWidth:canvas.width,canvasHeight:canvas.height,
    viewportWidth:bounds.width,viewportHeight:bounds.height,inputWidth:input.width,inputHeight:input.height,
    renderObjectCount:plugin.canvas3d.getRenderObjects().length,
    picked,dataUrl:capture()}};
}}catch(error){{window.audit={{status:'failed',error:String(error)}}}}}});
</script>"""
    return MOLSTAR_PREFIX + body


def run_browser(browser: str, node_root: Path, fixture: Path, output: Path) -> dict[str, Any]:
    """Run both pages in one disposable browser session."""

    session = f"molgfx-web-picking-{os.getpid()}"
    result: dict[str, Any] = {"status": "passed", "engines": {}}
    with tempfile.TemporaryDirectory(prefix="molgfx-web-picking-pages-") as directory:
        webroot = Path(directory)
        (webroot / "node_modules").symlink_to(node_root / "node_modules", target_is_directory=True)
        class QuietHandler(SimpleHTTPRequestHandler):
            def log_message(self, _format: str, *_args: Any) -> None:
                return

        server = ThreadingHTTPServer(
            ("127.0.0.1", 0), partial(QuietHandler, directory=str(webroot))
        )
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            for engine in ("ngl", "molstar"):
                page = webroot / f"{engine}.html"
                page.write_text(page_html(engine, fixture.read_text(encoding="utf-8")), encoding="utf-8")
                browser_json(browser, session, ["open", f"http://127.0.0.1:{server.server_port}/{page.name}"])
                wait_audit(browser, session)
                record = json.loads(browser_result(browser, session, "JSON.stringify(window.audit)"))
                if record.get("dataUrl"):
                    image_path = output / f"{engine}-picking.png"
                    record["image"] = capture_pick_image(browser, session, image_path)
                if record.get("status") != "passed":
                    result["status"] = "passed-with-failures"
                record.pop("dataUrl", None)
                result["engines"][engine] = record
        finally:
            close_browser(browser, session)
            server.shutdown()
            thread.join(timeout=5)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--node-root", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, default=Path("benchmarks/scenes/1BNA.cif"))
    parser.add_argument("--browser-use", default=shutil.which("browser-use"))
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = args.repo.resolve()
    fixture = args.fixture if args.fixture.is_absolute() else root / args.fixture
    if not args.browser_use:
        result: dict[str, Any] = {"schema": 1, "status": "unavailable", "error": "browser-use not found"}
    else:
        with tempfile.TemporaryDirectory(prefix="molgfx-web-picking-images-") as directory:
            result = {
                "schema": 1,
                "scope": "actual browser canvas picking; no cross-engine pixel equivalence claim",
                "fixture": str(fixture.relative_to(root)),
                **run_browser(args.browser_use, args.node_root.resolve(), fixture.resolve(), Path(directory)),
            }
    encoded = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        print(encoded, end="")
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
