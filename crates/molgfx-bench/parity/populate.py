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
            x, y, z = ((i % side - side / 2) * 4,
                       ((i // side) % side - side / 2) * 4,
                       (i // (side * side) - side / 2) * 4)
            out.write(f"HETATM {i + 1} He HE . HE A 1 . ? {x:.3f} {y:.3f} {z:.3f} 1 0 {i + 1} HE A HE 1\n")
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
    struct.pack_into("<f", header, 216, math.sqrt(sum((v - mean) ** 2 for v in values) / len(values)))
    with path.open("wb") as out:
        out.write(header)
        out.write(struct.pack("<" + "f" * len(values), *values))


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
    parser.add_argument("--lock", action="store_true", help="Explicitly record newly fetched hashes in the manifest")
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    populate(manifest, args.cache, args.lock)
    if args.lock:
        args.manifest.write_text(json.dumps(manifest, indent=2) + "\n")
