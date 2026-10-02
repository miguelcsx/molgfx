# MolGFX

[Documentation](https://miguelcsx.github.io/molgfx/)

MolGFX is a semantic molecular-rendering library for Python, Rust, and WebGPU.
It renders structures supplied by [MolFrame](https://github.com/miguelcsx/molframe)
as interactive browser scenes or deterministic native images. MolGFX does not
parse, fetch, dock, simulate, or open an application window.

![MolGFX haemoglobin cartoon, heme, and local pocket](site/public/images/haemoglobin-pocket.png)

## Install

```bash
python -m pip install --upgrade molgfx
```

## Render a structure

```python
import molframe
import molgfx

# Infer bonds so atom-and-bond representations have sticks to draw.
structure = molframe.read("structure.cif").infer_bonds()
scene = molgfx.Scene(structure)
scene.add(molgfx.rep.cartoon(target=molgfx.sel.protein(), color=molgfx.color.chain()))
scene.add(molgfx.rep.ball_and_stick(target=molgfx.sel.ligands()))

molgfx.Renderer().render_image(scene, size=(1920, 1080)).save("structure.png")
```

Targets are MolFrame queries, and a small command language works around them
through `molgfx.Session`:

```text
show cartoon, protein
select pocket, byres (within 5 of resname HEM) and protein
show licorice, $pocket
color orange, $pocket
```

MolGFX is an engine: it opens no window and ships no panels or notebook widgets.
A page, a notebook or an application draws what it likes around the scene and
the rendered frames. The camera primitives (`Scene.frame`, `Camera.project`,
`Camera.ray` and the arcball, orbit and fly controllers) are what such a host
builds navigation and overlays from.

[Writing queries and commands](https://miguelcsx.github.io/molgfx/docs/commands/queries)
walks through both, and the
[MolFrame query reference](https://miguelcsx.github.io/molframe/docs/query-language/)
lists every keyword.

## Status

Beta. Public APIs are available through the `molgfx` facade; MolFrame provides
structure input and molecular selection semantics.

## License

[MIT](LICENSE)
