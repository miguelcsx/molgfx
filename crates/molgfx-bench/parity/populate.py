"""Populate content-addressed parity inputs. Network/hash failures are fatal."""

import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
import struct
import urllib.request

ATOM_COLUMNS = """group_PDB id type_symbol label_atom_id label_alt_id label_comp_id
label_asym_id label_entity_id label_seq_id pdbx_PDB_ins_code Cartn_x Cartn_y
Cartn_z occupancy B_iso_or_equiv auth_seq_id auth_comp_id auth_asym_id
 auth_atom_id pdbx_PDB_model_num""".split()


def lattice(path, count):
    side = math.ceil(count ** (1 / 3))
    with path.open("w") as out:
        out.write("data_lattice\n#\nloop_\n_entity.id\n_entity.type\n1 non-polymer\n#\n")
        out.write("loop_\n" + "".join("_atom_site." + x + "\n" for x in ATOM_COLUMNS))
        for i in range(count):
            x, y, z = (
                (i % side - side / 2) * 4,
                ((i // side) % side - side / 2) * 4,
                (i // (side * side) - side / 2) * 4,
            )
            out.write(
                f"HETATM {i + 1} He HE . HE A 1 . ? {x:.3f} {y:.3f} {z:.3f} 1 0 {i + 1} HE A HE 1\n"
            )
        out.write("#\n")


def analytic_map(path, skew):
    n, extent = 32, 32.0
    gamma = math.radians(70 if skew else 90)
    header = bytearray(1024)
    struct.pack_into("<4i", header, 0, n, n, n, 2)
    struct.pack_into("<3i", header, 28, n, n, n)
    struct.pack_into("<6f", header, 40, extent, extent, extent, 90, 90, math.degrees(gamma))
    struct.pack_into("<3i", header, 64, 1, 2, 3)
    struct.pack_into("<3f", header, 196, -16, -16, -16)
    header[208:212], header[212:216] = b"MAP ", b"DA\x00\x00"
    values = []
    # Evaluate in physical world coordinates, not skew grid indices.
    for z in range(n):
        for y in range(n):
            for x in range(n):
                wx = -16 + x + y * math.cos(gamma)
                wy = -16 + y * math.sin(gamma)
                wz = -16 + z
                values.append(math.exp(-(wx * wx + wy * wy + wz * wz) / 32))
    mean = sum(values) / len(values)
    struct.pack_into("<3f", header, 76, min(values), max(values), mean)
    struct.pack_into(
        "<f", header, 216, math.sqrt(sum((v - mean) ** 2 for v in values) / len(values))
    )
    with path.open("wb") as out:
        out.write(header)
        out.write(struct.pack("<" + "f" * len(values), *values))


def _fmt(value):
    return f"{value:.3f}"


def _methyl_hydrogens(carbon, axis):
    """Three hydrogens 1.09 A from a tetrahedral carbon whose substituent lies along -axis."""
    ax = _unit(axis)
    helper = (0.0, 0.0, 1.0)
    side = _unit(_cross(ax, helper))
    up = _cross(ax, side)
    cos_t, sin_t = math.cos(math.radians(70.53)), math.sin(math.radians(70.53))
    out = []
    for k in range(3):
        phi = math.radians(120 * k + 90)
        perp = tuple(math.cos(phi) * s + math.sin(phi) * u for s, u in zip(side, up))
        out.append(tuple(c + 1.09 * (cos_t * a + sin_t * p) for c, a, p in zip(carbon, ax, perp)))
    return out


def _unit(v):
    n = math.sqrt(sum(x * x for x in v))
    return tuple(x / n for x in v)


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def bond_order_ligand(path):
    """One component LIG: ethyne, benzene, acetone, a Re-Re quadruple bond; ideal geometry.

    Bond orders ride on `_struct_conn` rows, the only per-file connectivity MolFrame reads."""
    atoms, bonds = [], []

    def atom(name, element, x, y, z):
        atoms.append((name, element, x, y, z))
        return name

    def bond(a, b, order, aromatic="N"):
        bonds.append((a, b, order, aromatic))

    # Ethyne H-C#C-H: C-H 1.09, C#C 1.20.
    h1, c1 = atom("HA1", "H", -9.69, 0, 0), atom("CA1", "C", -8.60, 0, 0)
    c2, h2 = atom("CA2", "C", -7.40, 0, 0), atom("HA2", "H", -6.31, 0, 0)
    bond(h1, c1, "SING"), bond(c1, c2, "TRIP"), bond(c2, h2, "SING")
    # Benzene: ring radius 1.39 (C-C), C-H 1.09 outward; Kekule orders, aromatic flag.
    ring = []
    for k in range(6):
        t = math.radians(60 * k)
        ring.append(atom(f"CB{k + 1}", "C", 1.39 * math.cos(t), 1.39 * math.sin(t), 0.0))
    for k in range(6):
        t = math.radians(60 * k)
        h = atom(f"HB{k + 1}", "H", 2.48 * math.cos(t), 2.48 * math.sin(t), 0.0)
        bond(ring[k], h, "SING")
        bond(ring[k], ring[(k + 1) % 6], "AROM", "Y")
    # Acetone: C=O 1.23, C-C 1.54, methyl C-H 1.09.
    cx = 9.0
    carbonyl = atom("CC1", "C", cx, 0.0, 0.0)
    oxygen = atom("OC1", "O", cx, 1.23, 0.0)
    bond(carbonyl, oxygen, "DOUB")
    for n, sign in enumerate((-1, 1), start=2):
        mx, my = cx + sign * 1.54 * math.sin(math.radians(60)), -1.54 * math.cos(math.radians(60))
        methyl = atom(f"CC{n}", "C", mx, my, 0.0)
        bond(carbonyl, methyl, "SING")
        for m, (hx, hy, hz) in enumerate(
            _methyl_hydrogens((mx, my, 0.0), (mx - cx, my, 0.0)), start=1
        ):
            hname = atom(f"HC{n}{m}", "H", hx, hy, hz)
            bond(methyl, hname, "SING")
    # Dirhenium core: Re-Re quadruple bond, 2.24 A.
    re1, re2 = atom("RE1", "Re", 0.0, 8.0, 0.0), atom("RE2", "Re", 2.24, 8.0, 0.0)
    bond(re1, re2, "QUAD")
    with path.open("w") as out:
        out.write("data_bond_orders\n#\nloop_\n_entity.id\n_entity.type\n1 non-polymer\n#\n")
        items = ["id", "conn_type_id", "pdbx_value_order"]
        for side in ("1", "2"):
            items += [
                f"ptnr{side}_label_asym_id",
                f"ptnr{side}_label_comp_id",
                f"ptnr{side}_label_seq_id",
                f"ptnr{side}_label_atom_id",
                f"ptnr{side}_auth_asym_id",
                f"ptnr{side}_auth_comp_id",
                f"ptnr{side}_auth_seq_id",
                f"ptnr{side}_auth_atom_id",
            ]
        out.write("loop_\n" + "".join(f"_struct_conn.{item}\n" for item in items))
        for i, (a, b, order, _) in enumerate(bonds, start=1):
            ends = " ".join(f"A LIG . {n} A LIG 1 {n}" for n in (a, b))
            out.write(f"c{i} covale {order} {ends}\n")
        out.write("#\nloop_\n" + "".join("_atom_site." + x + "\n" for x in ATOM_COLUMNS))
        for i, (name, element, x, y, z) in enumerate(atoms, start=1):
            out.write(
                f"HETATM {i} {element} {name} . LIG A 1 . ? {_fmt(x)} {_fmt(y)} {_fmt(z)} 1 0 1 LIG A {name} 1\n"
            )
        out.write("#\n")


RADIUS = {"H": 1.1, "C": 1.7, "N": 1.55, "O": 1.52, "S": 1.8, "P": 1.8, "FE": 1.4}


def _first_model_atoms(path):
    columns, atoms = [], []
    in_loop = False
    for line in path.read_text().splitlines():
        if line.startswith("_atom_site."):
            columns.append(line.strip().split(".", 1)[1])
            in_loop = True
        elif in_loop and columns and not line.startswith(("_", "#", "loop_")):
            fields = line.split()
            if len(fields) < len(columns):
                continue
            row = dict(zip(columns, fields))
            if row.get("pdbx_PDB_model_num", "1") == "1":
                atoms.append(
                    (
                        row["type_symbol"].upper(),
                        float(row["Cartn_x"]),
                        float(row["Cartn_y"]),
                        float(row["Cartn_z"]),
                    )
                )
        elif in_loop and (line.startswith("#") or line.startswith("loop_")):
            break
    return atoms


def model_density(path, structure_path):
    """Gaussian model density of a structure on a skewed (gamma 80) 0.7 A lattice."""
    spacing, gamma = 0.7, math.radians(80)
    atoms = _first_model_atoms(structure_path)
    cos_g, sin_g = math.cos(gamma), math.sin(gamma)

    def to_index(x, y, z):
        # world = i*s*(1,0,0) + j*s*(cos g, sin g, 0) + k*s*(0,0,1)
        j = y / (sin_g * spacing)
        return (x / spacing - j * cos_g, j, z / spacing)

    pad = 4.0 / spacing
    indices = [to_index(x, y, z) for _, x, y, z in atoms]
    lo = [math.floor(min(p[a] for p in indices) - pad) for a in range(3)]
    hi = [math.ceil(max(p[a] for p in indices) + pad) for a in range(3)]
    dims = [hi[a] - lo[a] + 1 for a in range(3)]
    grid = [0.0] * (dims[0] * dims[1] * dims[2])
    reach = 5
    for (element, x, y, z), (fi, fj, fk) in zip(atoms, indices):
        sigma = 0.6 * RADIUS.get(element, 1.7) / 1.7
        inv = 1.0 / (2 * sigma * sigma)
        ci, cj, ck = round(fi), round(fj), round(fk)
        for k in range(ck - reach, ck + reach + 1):
            for j in range(cj - reach, cj + reach + 1):
                for i in range(ci - reach, ci + reach + 1):
                    wx = (i + j * cos_g) * spacing
                    wy = j * sin_g * spacing
                    wz = k * spacing
                    d2 = (wx - x) ** 2 + (wy - y) ** 2 + (wz - z) ** 2
                    if d2 > 9 * sigma * sigma:
                        continue
                    a, b, c = i - lo[0], j - lo[1], k - lo[2]
                    if 0 <= a < dims[0] and 0 <= b < dims[1] and 0 <= c < dims[2]:
                        grid[(c * dims[1] + b) * dims[0] + a] += math.exp(-d2 * inv)
    header = bytearray(1024)
    struct.pack_into("<4i", header, 0, *dims, 2)
    struct.pack_into("<3i", header, 16, 0, 0, 0)
    struct.pack_into("<3i", header, 28, *dims)
    struct.pack_into("<6f", header, 40, *(d * spacing for d in dims), 90, 90, math.degrees(gamma))
    struct.pack_into("<3i", header, 64, 1, 2, 3)
    mean = sum(grid) / len(grid)
    struct.pack_into("<3f", header, 76, min(grid), max(grid), mean)
    struct.pack_into(
        "<3f",
        header,
        196,
        *((lo[0] + lo[1] * cos_g) * spacing, lo[1] * sin_g * spacing, lo[2] * spacing),
    )
    header[208:212], header[212:216] = b"MAP ", b"DA\x00\x00"
    struct.pack_into("<f", header, 216, math.sqrt(sum((v - mean) ** 2 for v in grid) / len(grid)))
    with path.open("wb") as out:
        out.write(header)
        out.write(struct.pack("<" + "f" * len(grid), *grid))


def populate(manifest, root, lock):
    root.mkdir(parents=True, exist_ok=True)
    for fixture in manifest["fixtures"]:
        path = root / fixture["file"]
        if not path.exists():
            if "url" in fixture:
                with urllib.request.urlopen(fixture["url"], timeout=120) as response:
                    data = response.read()
                if fixture.get("compression") == "gzip":
                    data = gzip.decompress(data)
                path.write_bytes(data)
            elif fixture["generator"] == "helium_lattice":
                lattice(path, fixture["atoms"])
            elif fixture["generator"] == "world_gaussian":
                analytic_map(path, fixture["skew"])
            elif fixture["generator"] == "bond_order_ligand":
                bond_order_ligand(path)
            elif fixture["generator"] == "model_density":
                model_density(path, root / fixture["structure"])
            else:
                raise ValueError("Unknown fixture generator: " + fixture["generator"])
        digest = hashlib.file_digest(path.open("rb"), "sha256").hexdigest()
        if lock:
            fixture["sha256"] = digest
        elif digest != fixture["sha256"]:
            raise ValueError(f"Fixture hash mismatch: {fixture['id']}: {digest}")
        print(f"{fixture['id']} {digest} {path.stat().st_size}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("cache", type=Path)
    parser.add_argument(
        "--lock", action="store_true", help="Explicitly record newly fetched hashes in the manifest"
    )
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    populate(manifest, args.cache, args.lock)
    if args.lock:
        args.manifest.write_text(json.dumps(manifest, indent=2) + "\n")
