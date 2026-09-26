"""Render a real PNG through an installed wheel.

Not a unittest (the name is not `test*.py`): it needs a graphics API, which the
workflow provides, and it must run against the wheel, not a development build.
"""

import json
import os
import sys

import molframe
import molgfx

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
"""

info = molgfx.system_info()
print(json.dumps(info, indent=2))
if sys.platform.startswith("linux") and "gl" not in info["compiled_backends"]:
    sys.exit("the Linux wheel was built without the OpenGL backend")

expected_backend = os.environ.get("WGPU_BACKEND")
if expected_backend:
    discovered = {adapter["backend"] for adapter in info["available_adapters"]}
    assert discovered == {expected_backend}, (expected_backend, discovered)

scene = molgfx.Scene(molframe.read(MMCIF, name="one.cif"))
scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
image = molgfx.Renderer().render_image(scene, size=(64, 64))
assert (image.width, image.height) == (64, 64)
pixels = image.pixels()
assert any(pixel != 0 for pixel in pixels), "render output is entirely transparent black"
assert image.png_bytes().startswith(b"\x89PNG")
print("rendered a non-empty 64x64 PNG")
