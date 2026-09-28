# MolGFX

[Documentation](https://miguelcsx.github.io/molgfx/) · [Runnable Workbench notebook](examples/workbench.ipynb)

MolGFX is a semantic molecular-rendering library for Python, Rust, and WebGPU.
It renders structures supplied by [MolFrame](https://github.com/miguelcsx/molframe)
as interactive browser scenes or deterministic native images. MolGFX does not
parse, fetch, dock, simulate, or open an application window.

![MolGFX haemoglobin cartoon, heme, and local pocket](site/public/images/haemoglobin-pocket.png)

## Install

```bash
python -m pip install --upgrade "molgfx[jupyter]>=0.3.0"
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

Use `molgfx.viewer.Workbench` in Jupyter for a browser WebGPU canvas, selection
commands, completion, history, and undo/redo. See the
[Workbench notebook](examples/workbench.ipynb) for haemoglobin (PDB 4HHB).

Targets are MolFrame queries, and the Workbench console speaks a small command
language around them:

```text
show cartoon, protein
select pocket, byres (within 5 of resname HEM) and protein
show licorice, $pocket
color orange, $pocket
```

[Writing queries and commands](https://miguelcsx.github.io/molgfx/docs/commands/queries)
walks through both, and the
[MolFrame query reference](https://miguelcsx.github.io/molframe/docs/query-language/)
lists every keyword.

## Status

Beta. Public APIs are available through the `molgfx` facade; MolFrame provides
structure input and molecular selection semantics.

## License

[MIT](LICENSE)
