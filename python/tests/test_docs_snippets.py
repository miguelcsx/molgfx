"""Every Python fence in the README and docs runs, page by page, on a synthetic entry.

Fences of one page run in order as one script, so a page reads top to
bottom. A fence tagged ```python no-run``` is skipped on purpose. Entries are
synthetic: `molframe.read` and `urlopen` return an in-memory structure, and
image sizes are shrunk so the test stays fast.
"""

import contextlib
import io
import os
import re
import runpy
import tempfile
import unittest
import urllib.request
from collections.abc import Generator
from pathlib import Path
from unittest import mock

import molframe

import molgfx

ROOT = Path(__file__).resolve().parents[2]
PAGES = [ROOT / "README.md", *sorted((ROOT / "site/content/docs").rglob("*.mdx"))]
FENCE = re.compile(r"```python([^\n]*)\n(.*?)```", re.DOTALL)
SIZE = re.compile(r"size=\(\d+, \d+\)")
_PROTEIN = ("N", "CA", "C", "O")
_HEADER = """data_docs
loop_
_entity.id
_entity.type
1 polymer
2 polymer
3 non-polymer
4 non-polymer
5 polymer
6 branched
loop_
_entity_poly.entity_id
_entity_poly.type
1 polypeptide(L)
2 polypeptide(L)
5 polydeoxyribonucleotide
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.auth_asym_id
_atom_site.auth_seq_id
_atom_site.auth_comp_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
"""
# residue, label asym, entity, author chain
_Row = tuple[str, str, str, int, int, str, int, tuple[float, float, float]]


def _entry() -> bytes:
    """Two protein chains, a heme and a ligand in chain A, DNA chain C and a glycan chain D."""
    rows: list[_Row] = []
    for chain, entity, offset in (("A", 1, 0.0), ("B", 2, 20.0)):
        for number in range(1, 7):
            for index, atom in enumerate(_PROTEIN):
                xyz = (3.8 * number + 1.2 * index, offset, 0.0)
                rows.append((atom, "ALA", chain, entity, number, chain, number, xyz))
    rows += [
        ("FE", "HEM", "C", 3, 0, "A", 101, (8.0, 4.0, 0.0)),
        ("C1", "HEM", "C", 3, 0, "A", 101, (9.0, 4.0, 0.0)),
        ("C1", "LIG", "D", 4, 0, "A", 102, (12.0, 4.0, 0.0)),
        ("O1", "LIG", "D", 4, 0, "A", 102, (13.0, 4.0, 0.0)),
    ]
    for number, name in enumerate(("DA", "DT"), start=1):
        for index, atom in enumerate(("P", "O5'", "C5'", "C4'", "O4'", "C3'", "C2'", "C1'", "N9")):
            xyz = (3.0 * index, 40.0 + 6.0 * number, 0.0)
            rows.append((atom, name, "E", 5, number, "C", number, xyz))
    rows += [
        ("C1", "NAG", "F", 6, 1, "D", 1, (0.0, 60.0, 0.0)),
        ("C2", "NAG", "F", 6, 1, "D", 1, (1.5, 60.0, 0.0)),
    ]
    lines: list[str] = []
    for serial, (atom, residue, asym, entity, seq, auth, number, xyz) in enumerate(rows, start=1):
        group = "ATOM" if entity in {1, 2, 5} else "HETATM"
        element = "FE" if atom == "FE" else atom[0]
        x, y, z = xyz
        lines.append(
            f"{group} {serial} {element} {atom} {residue} {asym} {entity} {seq or '.'} "
            f"{auth} {number} {residue} {x:.3f} {y:.3f} {z:.3f} 1.00 10.00"
        )
    return (_HEADER + "\n".join(lines) + "\n").encode()


TWO_ATOMS = (
    b"ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C  \n"
    b"ATOM      2  CA  ALA A   2       4.000   0.000   0.000  1.00 10.00           C  \n"
    b"END\n"
)


@contextlib.contextmanager
def _offline() -> Generator[None]:
    real = molframe.read

    def read(source: object, **_: object) -> molframe.Structure:
        two = isinstance(source, str) and source.endswith("two-atoms.pdb")
        return real(TWO_ATOMS if two else _entry(), name="entry.cif")

    def urlopen(*_args: object, **_kwargs: object) -> io.BytesIO:
        return io.BytesIO(_entry())

    with (
        mock.patch.object(molframe, "read", read),
        mock.patch.object(urllib.request, "urlopen", urlopen),
    ):
        yield


def _fences(page: Path) -> list[tuple[str, str]]:
    return [(meta.strip(), code) for meta, code in FENCE.findall(page.read_text())]


class DocsSnippetTests(unittest.TestCase):
    """The documented Python runs as written."""

    def test_pages_are_discovered(self) -> None:
        self.assertGreater(sum(len(_fences(page)) for page in PAGES), 30)

    def test_every_fence_runs(self) -> None:
        cwd = Path.cwd()
        for page in PAGES:
            runnable = [code for meta, code in _fences(page) if meta != "no-run"]
            if not runnable:
                continue
            script = "\n".join(SIZE.sub("size=(64, 48)", code) for code in runnable)
            with (
                self.subTest(page=str(page.relative_to(ROOT))),
                tempfile.TemporaryDirectory() as tmp,
            ):
                path = Path(tmp) / "snippet.py"
                path.write_text(script)
                os.chdir(tmp)
                try:
                    with _offline():
                        runpy.run_path(str(path))
                finally:
                    os.chdir(cwd)

    def test_names_resolve_to_the_public_api(self) -> None:
        for name in ("by_array", "place", "playback"):
            self.assertTrue(hasattr(molgfx, name))


if __name__ == "__main__":
    unittest.main()
