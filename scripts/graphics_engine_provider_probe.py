"""Provider-only probes for the structural data consumed by renderers."""

from __future__ import annotations

import re
import struct
import tempfile
from pathlib import Path
from typing import Any, Callable


PDBIOX_PDB = b"""HEADER    PDBIOX GRAPHICS INPUT PROBE\nATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 10.00           N  \nATOM      2  CA  ALA A   1       1.450   0.000   0.000  1.00 11.00           C  \nATOM      3  C   ALA A   1       2.000   1.400   0.000  1.00 12.00           C  \nATOM      4  O   ALA A   1       1.500   2.500   0.000  1.00 13.00           O  \nEND\n"""
PDBIOX_CIF = b"data_probe\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n_atom_site.pdbx_PDB_ins_code\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n_atom_site.auth_comp_id\n_atom_site.auth_asym_id\n_atom_site.auth_atom_id\n_atom_site.pdbx_PDB_model_num\nATOM 1 C CA . GLY A 1 1 ? 0 0 0 1.0 10.0 1 GLY A CA 1\n"


PDBIOX_MOL = """CO
  pdbiox
comment
  2  1  0  0  0  0  0  0  0  0999 V2000
    0.0000    0.0000    0.0000 C   0  0
    1.2300    0.0000    0.0000 O   0  0
  1  2  2  0
M  END
"""
PDBIOX_MOL2 = """@<TRIPOS>MOLECULE
ligand
2 1 1 0 0
SMALL
USER_CHARGES
SYSTEM
comment
@<TRIPOS>ATOM
10 C1 0.0 1.0 2.0 C.ar 1 LIG -0.125 BACKBONE
20 N1 1.5 1.0 2.0 N.am 1 LIG 0.125
@<TRIPOS>BOND
7 10 20 ar TYPE1
@<TRIPOS>SUBSTRUCTURE
1 LIG 1 GROUP
"""

PDBIOX_PQR = b"""ATOM      1  N   ALA A   1      11.104  13.207   9.124 -0.3000 1.5500
END
"""
PDBIOX_PDBQT = b"""ROOT
ATOM      1  N   LIG A   1      11.104  13.207   9.124  1.00  0.00      -0.300 N
ENDROOT
TORSDOF 0
"""
PDBIOX_MMTF_CIF = b"""data_mmtf_probe
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_alt_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.pdbx_PDB_ins_code
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
_atom_site.occupancy
_atom_site.B_iso_or_equiv
_atom_site.pdbx_formal_charge
_atom_site.auth_seq_id
_atom_site.auth_comp_id
_atom_site.auth_asym_id
_atom_site.auth_atom_id
_atom_site.pdbx_PDB_model_num
ATOM 1 N N . ALA A 1 1 ? 0.0 0.0 0.0 1.0 10.0 0 1 ALA A N 1
ATOM 2 C CA . ALA A 1 1 ? 1.45 0.0 0.0 1.0 11.0 0 1 ALA A CA 1
ATOM 3 C C . ALA A 1 1 ? 2.0 1.4 0.0 1.0 12.0 0 1 ALA A C 1
ATOM 4 O O . ALA A 1 1 ? 1.5 2.5 0.0 1.0 13.0 0 1 ALA A O 1
"""

TRAJECTORY_FORMAT_NAMES = (
    "Xtc", "Trr", "Dcd", "AmberNetcdf", "Tng", "Gsd", "H5md", "Trz",
    "Namd", "AmberRestart", "AmberAscii", "Gro", "Xyz", "Aims", "Txyz",
    "DlPolyConfig", "DlPolyHistory", "CharmmCard", "Gamess", "LammpsDump",
    "Gromos11", "Dms",
)


def public_symbols(module: Any) -> list[str]:
    """Return the namespace contract without counting private implementation names."""

    exported = getattr(module, "__all__", ())
    if exported:
        return sorted(str(name) for name in exported if not str(name).startswith("_"))
    return sorted(name for name in dir(module) if not name.startswith("_"))


def symbol_capability(module: Any, patterns: tuple[str, ...]) -> dict[str, Any]:
    """Record a public capability search, including an explicit absence."""

    names = public_symbols(module)
    matches = [
        name
        for name in names
        if any(re.search(pattern, name, flags=re.IGNORECASE) for pattern in patterns)
    ]
    return {
        "status": "available" if matches else "not-exposed",
        "matches": matches,
        "public_symbol_count": len(names),
    }


def chemistry_molecule_formats(pdbiox: Any) -> dict[str, Any]:
    """Exercise chemistry-format helpers without treating them as structure readers."""

    mol = pdbiox.chem.parse_mol_record(PDBIOX_MOL)
    sdf = pdbiox.chem.write_sdf([mol, mol])
    mol2 = pdbiox.chem.parse_mol2_record(PDBIOX_MOL2)
    return {
        "status": "available",
        "mol_atoms": len(mol.molecule.atoms),
        "mol_bonds": len(mol.molecule.bonds),
        "sdf_records": len(pdbiox.chem.parse_sdf_records(sdf)),
        "mol2_atoms": len(mol2.molecule.atoms),
        "mol2_bonds": len(mol2.molecule.bonds),
        "chemistry_public_symbol_count": len(public_symbols(pdbiox.chem)),
    }


def _structure_format_summary(report: Any) -> dict[str, Any]:
    """Summarize a structure reader without treating it as a renderer."""

    structure = report.structure
    return {
        "status": "available",
        "kind": "structure",
        "atoms": structure.atom_count,
        "models": structure.model_count,
        "findings": len(report.findings),
    }


def structure_format_envelope(pdbiox: Any, structure: Any) -> dict[str, Any]:
    """Exercise viewer-facing structure formats and classify GRO correctly."""

    options = pdbiox.ReadOptions.standard()
    pdb_namespace = getattr(pdbiox, "pdb", None)
    readers = {
        "PDBQT": getattr(pdb_namespace, "read_pdbqt", None),
        "PQR": getattr(pdb_namespace, "read_pqr", None),
    }
    writers = {
        "PDBQT": getattr(pdb_namespace, "write_pdbqt", None),
        "PQR": getattr(pdb_namespace, "write_pqr", None),
    }
    results: dict[str, Any] = {}
    inputs = {"PDBQT": PDBIOX_PDBQT, "PQR": PDBIOX_PQR}
    for name, reader in readers.items():
        if reader is None:
            results[name] = {"status": "not-exposed", "kind": "structure"}
            continue
        report = reader(inputs[name], options)
        results[name] = _structure_format_summary(report)
        writer = writers[name]
        write_options = getattr(pdb_namespace, "PdbOptions", None)
        if callable(writer) and callable(write_options):
            encoded = writer(report.structure, write_options())
            round_trip = reader(encoded.encode(), options)
            results[name].update(
                {
                    "encoded_bytes": len(encoded),
                    "writer_round_trip_atoms": round_trip.structure.atom_count,
                }
            )

    if pdb_namespace is None or not all(
        callable(getattr(pdb_namespace, name, None))
        for name in ("MmtfEntityMetadata", "MmtfGroupMetadata", "MmtfMetadata", "MmtfOptionalField", "with_mmtf_metadata", "write_mmtf", "read_mmtf")
    ):
        results["MMTF"] = {"status": "not-exposed", "kind": "structure"}
    else:
        metadata = pdb_namespace.MmtfMetadata(
            groups=[
                pdb_namespace.MmtfGroupMetadata(
                    "ALA", ["N", "CA", "C", "O"], ["N", "C", "C", "O"], "A", "L-peptide linking"
                )
            ],
            entities=[pdb_namespace.MmtfEntityMetadata("alanine", "polymer", "A")],
            optional_fields=[
                pdb_namespace.MmtfOptionalField.BFactor,
                pdb_namespace.MmtfOptionalField.Occupancy,
                pdb_namespace.MmtfOptionalField.AtomId,
                pdb_namespace.MmtfOptionalField.SequenceIndex,
                pdb_namespace.MmtfOptionalField.ChainName,
                pdb_namespace.MmtfOptionalField.EntityList,
            ],
        )
        mmtf_source = pdbiox.read_bytes(
            PDBIOX_MMTF_CIF, options, name="graphics-input-probe-mmtf.cif"
        ).structure
        attached = pdb_namespace.with_mmtf_metadata(mmtf_source, metadata)
        encoded = pdb_namespace.write_mmtf(attached)
        decoded = pdb_namespace.read_mmtf(encoded)
        results["MMTF"] = _structure_format_summary(decoded)
        results["MMTF"].update({"encoded_bytes": len(encoded), "metadata": True})

    gro_namespace = getattr(getattr(pdbiox, "traj", None), "format_gro", None)
    if gro_namespace is None:
        results["GRO"] = {"status": "not-exposed", "kind": "trajectory-records"}
    else:
        frame = gro_namespace.GroFrame(
            "graphics-input-probe",
            [
                gro_namespace.GroAtom(1, "ALA", "N", 1, [0.0, 0.0, 0.0], None),
                gro_namespace.GroAtom(1, "ALA", "CA", 2, [1.5, 0.0, 0.0], None),
            ],
            [10.0, 11.0, 12.0],
        )
        encoded = gro_namespace.write_gro([frame])
        decoded = gro_namespace.parse_gro_records(encoded)
        results["GRO"] = {
            "status": "available",
            "kind": "trajectory-records",
            "frames": len(decoded),
            "atoms": len(decoded[0].atoms),
            "box_values": decoded[0].box_values,
            "encoded_bytes": len(encoded),
        }
    return results


def trajectory_format_envelope(pdbiox: Any) -> dict[str, Any]:
    """Exercise normalized and format-specific trajectory input paths."""

    import numpy as np

    traj_namespace = getattr(pdbiox, "traj", None)
    trajectory_type = getattr(traj_namespace, "Trajectory", None)
    write_options_type = getattr(traj_namespace, "TrajectoryWriteOptions", None)
    format_type = getattr(traj_namespace, "TrajectoryFormat", None)
    if not all(callable(value) for value in (trajectory_type, write_options_type)):
        return {
            "status": "not-exposed",
            "format_enum": {
                name: {"status": "not-exposed"} for name in TRAJECTORY_FORMAT_NAMES
            },
            "round_trips": {},
        }

    format_enum = {
        name: {
            "status": "available" if getattr(format_type, name, None) is not None else "not-exposed",
            "kind": "trajectory-format-enum",
        }
        for name in TRAJECTORY_FORMAT_NAMES
    }
    trajectory = trajectory_type(
        np.ascontiguousarray(
            [
                [[0.0, 0.0, 0.0], [1.4, 0.0, 0.0]],
                [[0.1, 0.0, 0.0], [1.5, 0.0, 0.0]],
            ],
            dtype=np.float32,
        ),
        time=np.ascontiguousarray([0.0, 1.0], dtype=np.float64),
        steps=np.ascontiguousarray([0, 1], dtype=np.int64),
        copy=True,
    )
    writer_factories = {
        "XTC": ("xtc", "Xtc"),
        "TRR": ("trr", "Trr"),
        "DCD": ("dcd", "Dcd"),
        "TNG": ("tng", "Tng"),
        "GSD": ("gsd", "Gsd"),
        "H5MD": ("h5md", "H5md"),
        "TRZ": ("trz", "Trz"),
        "AMBER-NETCDF": ("amber_netcdf", "AmberNetcdf"),
    }
    round_trips: dict[str, Any] = {}
    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="molgfx-pdbiox-trajectory-formats-") as directory:
        root = Path(directory)
        for name, (factory_name, format_name) in writer_factories.items():
            factory = getattr(write_options_type, factory_name, None)
            format_value = getattr(format_type, format_name, None)
            record: dict[str, Any] = {
                "format": format_name,
                "writer_option": callable(factory),
                "reader_format": format_value is not None,
            }
            path = root / name.lower().replace("-", "_")
            if not callable(factory) or format_value is None:
                record["status"] = "not-exposed"
            else:
                try:
                    options = factory(1.0) if factory_name == "gsd" else factory()
                except Exception as error:
                    record.update(
                        {
                            "status": "rejected",
                            "phase": "options",
                            "error": f"{type(error).__name__}: {error}"[:300],
                        }
                    )
                    failures.append(name)
                else:
                    try:
                        trajectory.write(path, options=options)
                    except Exception as error:
                        record.update(
                            {
                                "status": "rejected",
                                "phase": "write",
                                "error": f"{type(error).__name__}: {error}"[:300],
                            }
                        )
                        failures.append(name)
                    else:
                        record["bytes"] = path.stat().st_size
                        try:
                            restored = trajectory_type.read(path, format=format_value)
                        except Exception as error:
                            record.update(
                                {
                                    "status": "rejected",
                                    "phase": "read",
                                    "error": f"{type(error).__name__}: {error}"[:300],
                                }
                            )
                            failures.append(name)
                        else:
                            record.update(
                                {
                                    "status": "available",
                                    "xyz_shape": list(restored.xyz.shape),
                                    "steps": restored.steps.tolist()
                                    if restored.steps is not None
                                    else None,
                                }
                            )
            round_trips[name] = record

    xyz_namespace = getattr(traj_namespace, "format_xyz", None)
    if xyz_namespace is not None and all(
        callable(getattr(xyz_namespace, name, None))
        for name in ("XyzAtom", "XyzFrame", "parse_xyz", "write_xyz")
    ):
        frames = [
            xyz_namespace.XyzFrame(
                "graphics-input-probe",
                [
                    xyz_namespace.XyzAtom("C", (0.0, 0.0, 0.0)),
                    xyz_namespace.XyzAtom("O", (1.4, 0.0, 0.0)),
                ],
            )
        ]
        encoded = xyz_namespace.write_xyz(frames)
        decoded = xyz_namespace.parse_xyz(encoded)
        round_trips["XYZ-records"] = {
            "status": "available",
            "bytes": len(encoded),
            "frames": len(decoded) if decoded is not None else 0,
            "atoms": len(decoded[0].atoms) if decoded else 0,
        }
    else:
        round_trips["XYZ-records"] = {"status": "not-exposed"}

    return {
        "status": "available",
        "format_enum": format_enum,
        "format_enum_count": len(format_enum),
        "round_trips": round_trips,
        "round_trip_failures": failures,
        "contract": "TrajectoryFormat enum is exposed for all listed families; actual binary round trips are recorded individually",
    }


def _write_extra_mrc_mode(path: Path, mode: int, size: int = 16) -> None:
    """Write a valid scalar/complex MRC fixture for provider-only mode checks."""

    from graphics_engine_volume_format_probe import cube_values

    values = cube_values(size)
    header = bytearray(1024)
    struct.pack_into("<3i", header, 0, size, size, size)
    struct.pack_into("<i", header, 12, mode)
    struct.pack_into("<3i", header, 28, size, size, size)
    struct.pack_into("<3f", header, 40, float(size), float(size), float(size))
    struct.pack_into("<3f", header, 52, 90.0, 90.0, 90.0)
    struct.pack_into("<3i", header, 64, 1, 2, 3)
    struct.pack_into("<3f", header, 76, min(values), max(values), sum(values) / len(values))
    header[208:216] = b"MAP " + b"DD\x00\x00"
    if mode == 6:
        encoded = struct.pack(f"<{len(values)}H", *(round(value * 1000) for value in values))
    elif mode == 12:
        encoded = b"".join(struct.pack("<e", value) for value in values)
    elif mode == 101:
        encoded_values = [max(0, min(15, round(value * 15))) for value in values]
        packed = bytearray((len(encoded_values) + 1) // 2)
        for index, value in enumerate(encoded_values):
            packed[index // 2] |= value << (4 * (index % 2))
        encoded = bytes(packed)
    elif mode == 3:
        encoded = b"".join(struct.pack("<2h", round(value * 1000), 0) for value in values)
    elif mode == 4:
        encoded = b"".join(struct.pack("<2f", value, 0.0) for value in values)
    else:
        raise ValueError(f"unsupported provider test mode {mode}")
    path.write_bytes(header + encoded)


def volume_format_envelope(pdbiox: Any) -> dict[str, Any]:
    """Compare the provider's density-input contract with the shared volume cases."""

    from graphics_engine_volume_format_probe import (
        write_brix,
        write_cube,
        write_dx,
        write_dsn6,
        write_mrc,
        write_mrc_integer,
    )

    density_map = getattr(getattr(pdbiox, "xtal", None), "DensityMap", None)
    reader = getattr(density_map, "from_mrc_bytes", None)
    xtal_symbols = public_symbols(getattr(pdbiox, "xtal", object()))
    dedicated_symbols = sorted(
        name
        for name in xtal_symbols
        if re.search(r"mrc|ccp4|cube|dx|dsn6|brix|xplor|cns", name, re.IGNORECASE)
    )
    cases = (
        "mrc",
        "map",
        "ccp4",
        "cube",
        "dx",
        "dsn6",
        "brix",
        "mrc-mode1",
        "mrc-mode0",
    )
    results: dict[str, Any] = {}
    with tempfile.TemporaryDirectory(prefix="molgfx-pdbiox-volume-formats-") as directory:
        root = Path(directory)
        paths = {
            "mrc": root / "fixture.mrc",
            "map": root / "fixture.map",
            "ccp4": root / "fixture.ccp4",
            "cube": root / "fixture.cube",
            "dx": root / "fixture.dx",
            "dsn6": root / "fixture.dsn6",
            "brix": root / "fixture.brix",
            "mrc-mode1": root / "fixture-mode1.mrc",
            "mrc-mode0": root / "fixture-mode0.mrc",
        }
        write_mrc(paths["mrc"], (16, 16, 16))
        paths["map"].write_bytes(paths["mrc"].read_bytes())
        paths["ccp4"].write_bytes(paths["mrc"].read_bytes())
        write_cube(paths["cube"])
        write_dx(paths["dx"])
        write_dsn6(paths["dsn6"])
        write_brix(paths["brix"])
        write_mrc_integer(paths["mrc-mode1"], 1)
        write_mrc_integer(paths["mrc-mode0"], 0)

        for name in cases:
            payload = paths[name].read_bytes()
            record: dict[str, Any] = {
                "format": name,
                "bytes": len(payload),
                "dedicated_public_symbols": dedicated_symbols,
            }
            if callable(reader):
                try:
                    decoded = reader(payload)
                    record.update(
                        {
                            "status": "available",
                            "reader": "pdbiox.xtal.DensityMap.from_mrc_bytes",
                            "dimensions": list(decoded.dimensions),
                            "values": len(decoded.values),
                        }
                    )
                except Exception as error:
                    record.update(
                        {
                            "status": "rejected-by-mrc-reader",
                            "reader_error": f"{type(error).__name__}: {error}"[:300],
                        }
                    )
            else:
                record.update(
                    {
                        "status": "not-exposed",
                        "reason": "pdbiox.xtal.DensityMap.from_mrc_bytes is not exposed",
                    }
                )
            results[name] = record
        extra_modes: dict[str, Any] = {}
        for mode in (6, 12, 101, 3, 4):
            path = root / f"fixture-mode{mode}.mrc"
            _write_extra_mrc_mode(path, mode)
            payload = path.read_bytes()
            record = {"mode": mode, "bytes": len(payload)}
            if callable(reader):
                try:
                    decoded = reader(payload)
                    record.update(
                        {
                            "status": "available",
                            "dimensions": list(decoded.dimensions),
                            "values": len(decoded.values),
                        }
                    )
                except Exception as error:
                    record.update(
                        {
                            "status": "rejected",
                            "error": f"{type(error).__name__}: {error}"[:300],
                        }
                    )
            else:
                record.update({"status": "not-exposed"})
            extra_modes[str(mode)] = record
    return {
        "status": "available" if callable(reader) else "not-exposed",
        "contract": "MRC/CCP4 scalar bytes (modes 0, 1, 2, 6, 12 and 101) plus programmatic DensityGrid; no extension-dispatch reader",
        "reader_present": callable(reader),
        "dedicated_public_symbols": dedicated_symbols,
        "formats": results,
        "additional_mrc_modes": extra_modes,
    }


def render_input_envelope(pdbiox: Any, structure: Any) -> dict[str, Any]:
    """Exercise data objects that can be handed to a graphics engine."""

    import numpy as np

    trajectory = pdbiox.traj.Trajectory(
        np.ascontiguousarray(
            [
                [[0.0, 0.0, 0.0], [1.4, 0.0, 0.0]],
                [[0.1, 0.0, 0.0], [1.5, 0.0, 0.0]],
            ],
            dtype=np.float32,
        ),
        steps=np.ascontiguousarray([0, 1], dtype=np.int64),
        copy=True,
    )
    frame = trajectory.frame(1)

    grid_spec = pdbiox.analysis.DensityGridSpec(
        [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [2, 2, 2]
    )
    grid = pdbiox.analysis.density_map(
        structure.xyz,
        np.ones(structure.atom_count, dtype=np.float64),
        grid_spec,
    )

    cell = pdbiox.xtal.UnitCell([20.0, 20.0, 20.0], [90.0, 90.0, 90.0])
    density_map = pdbiox.xtal.DensityMap(
        [2, 2, 2],
        [0, 0, 0],
        [2, 2, 2],
        cell,
        [0.0, 0.0, 0.0],
        1,
        ["graphics-input-probe"],
        [],
        [float(value) for value in range(8)],
    )
    restored_map = pdbiox.xtal.DensityMap.from_mrc_bytes(density_map.to_mrc_bytes())
    ses = pdbiox.surface.solvent_excluded_surface(
        structure.xyz,
        np.full(structure.atom_count, 1.7, dtype=np.float32),
        1.4,
        0.5,
    )
    mesh = ses.indexed_mesh()
    with tempfile.TemporaryDirectory(prefix="molgfx-pdbiox-provider-") as directory:
        obj_path = Path(directory) / "surface.obj"
        pdbiox.surface.write_obj(str(obj_path), mesh)
        obj_bytes = obj_path.stat().st_size

    analysis_symbols = public_symbols(pdbiox.analysis)
    xtal_symbols = public_symbols(pdbiox.xtal)
    return {
        "trajectory": {
            "status": "available",
            "frames": len(trajectory),
            "atoms": trajectory.atom_count,
            "xyz_shape": list(trajectory.xyz.shape),
            "frame_one_atoms": frame.atom_count,
            "steps": trajectory.steps.tolist(),
        },
        "density_grid": {
            "status": "available",
            "shape": list(grid.spec.shape),
            "value_count": int(grid.density.size),
            "excluded_weight": grid.excluded_weight,
        },
        "mrc_density_map": {
            "status": "available",
            "dimensions": list(restored_map.dimensions),
            "value_count": int(restored_map.values.size),
            "mrc_bytes": len(density_map.to_mrc_bytes()),
            "statistics_count": restored_map.statistics().count,
        },
        "volume_format_envelope": volume_format_envelope(pdbiox),
        "trajectory_format_envelope": trajectory_format_envelope(pdbiox),
        "surface_mesh_obj": {
            "status": "available",
            "vertices": int(mesh.vertices.shape[0]),
            "faces": int(mesh.faces.shape[0]),
            "obj_bytes": obj_bytes,
            "boundary_edges": mesh.report.boundary_edges,
        },
        "structure_format_envelope": structure_format_envelope(pdbiox, structure),
        "crystallographic_transform": {
            "status": "available",
            "cartesian": cell.to_cartesian(
                np.ascontiguousarray([[0.25, 0.5, 0.75]], dtype=np.float64)
            ).tolist(),
            "space_group_symbols": [
                name for name in xtal_symbols if "space" in name.lower() or "symmetr" in name.lower()
            ],
        },
        "vector_field": symbol_capability(
            pdbiox,
            (
                r"vector[_ ]?field",
                r"field[_ ]?vector",
                r"field[_ ]?line",
                r"streamline",
                r"flow[_ ]?field",
            ),
        ),
        "orbital_or_wavefunction": symbol_capability(
            pdbiox,
            (r"orbital", r"wave.?function", r"electron.?density"),
        ),
        "volume_and_grid_symbols": {
            "analysis": symbol_capability(
                pdbiox.analysis, (r"density", r"grid", r"volume", r"voxel")
            ),
            "crystallography": symbol_capability(
                pdbiox.xtal, (r"density", r"map", r"volume", r"voxel")
            ),
        },
        "public_symbol_counts": {
            "analysis": len(analysis_symbols),
            "surface": len(public_symbols(pdbiox.surface)),
            "traj": len(public_symbols(pdbiox.traj)),
            "xtal": len(xtal_symbols),
        },
    }


def probe_pdbiox(
    run_probe: Callable[[str, Callable[[], Any]], dict[str, Any]],
    compact_error: Callable[[BaseException], str],
) -> dict[str, Any]:
    """Probe only data capabilities consumed by graphics engines."""

    try:
        import pdbiox
    except Exception as error:
        return {
            "status": "unavailable",
            "error": compact_error(error),
            "scope": "provider only; pdbiox is not a graphics engine",
        }

    try:
        options = pdbiox.ReadOptions.standard()
        report = pdbiox.read_bytes(PDBIOX_PDB, options, name="graphics-input-probe.pdb")
        structure = report.structure
        provider_probes = [
            run_probe(
                "coordinate-column",
                lambda: {
                    "shape": list(structure.xyz.shape),
                    "dtype": str(structure.xyz.dtype),
                },
            ),
            run_probe(
                "pdb-and-mmcif-input",
                lambda: {
                    "pdb_atoms": structure.atom_count,
                    "mmcif_atoms": pdbiox.read_bytes(
                        PDBIOX_CIF, options, name="graphics-input-probe.cif"
                    ).structure.atom_count,
                },
            ),
            run_probe(
                "hierarchy",
                lambda: {
                    "models": structure.model_count,
                    "chains": structure.chain_count,
                    "residues": structure.residue_count,
                    "atoms": structure.atom_count,
                },
            ),
            run_probe(
                "selection-and-spatial-query",
                lambda: {
                    "all": list(structure.select(pdbiox.Query("all")).indices),
                    "within": list(
                        structure.select(pdbiox.Query("within 2 of (name CA)")).indices
                    ),
                },
            ),
            run_probe(
                "bond-inference",
                lambda: repr(
                    pdbiox.infer_bonds(structure, pdbiox.BondInference.standard())
                )[:200],
            ),
            run_probe(
                "surface-and-crystallography-provider",
                lambda: {
                    "sasa_values": len(
                        pdbiox.surface.solvent_accessible_surface(
                            structure.xyz, structure.xyz[:, 0] * 0 + 1.7, 1.4, 32
                        )
                    ),
                    "ses_type": type(
                        pdbiox.surface.solvent_excluded_surface(
                            structure.xyz, structure.xyz[:, 0] * 0 + 1.7, 1.4, 0.5
                        )
                    ).__name__,
                    "unit_cell_type": type(
                        pdbiox.xtal.UnitCell([20.0, 20.0, 20.0], [90.0, 90.0, 90.0])
                    ).__name__,
                },
            ),
            run_probe("validation", lambda: len(structure.validate())),
            run_probe(
                "module-surface-inventory",
                lambda: {
                    module: len(public_symbols(getattr(pdbiox, module)))
                    for module in ("analysis", "surface", "traj", "xtal", "compare")
                    if getattr(pdbiox, module, None) is not None
                },
            ),
            run_probe(
                "renderer-input-envelope",
                lambda: render_input_envelope(pdbiox, structure),
            ),
            run_probe(
                "chemistry-molecule-format-input",
                lambda: chemistry_molecule_formats(pdbiox),
            ),
        ]
        return {
            "status": "passed",
            "scope": "provider only; pdbiox is not a graphics engine",
            "root_symbol_count": len(getattr(pdbiox, "__all__", ())),
            "structure": repr(structure),
            "findings": len(report.findings),
            "probes": provider_probes,
        }
    except Exception as error:
        return {
            "status": "failed",
            "scope": "provider only; pdbiox is not a graphics engine",
            "error": compact_error(error),
        }
