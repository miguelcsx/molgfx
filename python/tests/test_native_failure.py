"""A structure whose BCIF encoding fails raises the producer's own error."""

import ctypes
import unittest
from collections.abc import MutableSequence
from typing import Any

import molframe.errors
import numpy as np

import molgfx

CAPSULE_NAME = b"molframe.StructureSource.v2"
BASE_ABI_VERSION = 2
FAILURE_ABI_VERSION = 3
PARSE_ERROR_NUMBER = 1103
MESSAGE = b"forced encode failure"

_Select = ctypes.CFUNCTYPE(
    ctypes.c_int64,
    ctypes.c_void_p,
    ctypes.c_char_p,
    ctypes.c_size_t,
    ctypes.c_void_p,
    ctypes.c_size_t,
)
_Encode = ctypes.CFUNCTYPE(ctypes.c_int64, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t)
_Describe = ctypes.CFUNCTYPE(
    ctypes.c_int64,
    ctypes.c_void_p,
    ctypes.POINTER(ctypes.c_uint32),
    ctypes.c_void_p,
    ctypes.c_size_t,
)


class _Source(ctypes.Structure):
    """The producer's C layout: the base record plus the appended failure callback."""

    _fields_ = (
        ("abi_version", ctypes.c_uint32),
        ("coordinate_generation", ctypes.c_uint64),
        ("coordinates", ctypes.c_void_p),
        ("coordinate_rows", ctypes.c_size_t),
        ("atoms", ctypes.c_void_p),
        ("atom_count", ctypes.c_size_t),
        ("residue_atom_start", ctypes.c_void_p),
        ("residue_atom_start_len", ctypes.c_size_t),
        ("chain_residue_start", ctypes.c_void_p),
        ("chain_residue_start_len", ctypes.c_size_t),
        ("model_chain_start", ctypes.c_void_p),
        ("model_chain_start_len", ctypes.c_size_t),
        ("bonds", ctypes.c_void_p),
        ("bond_count", ctypes.c_size_t),
        ("select", _Select),
        ("encode_bcif", _Encode),
        ("describe_failure", _Describe),
    )


def _refuse_select(*_: object) -> int:
    """Answer no query; import never evaluates one."""
    return -1


def _refuse_encode(*_: object) -> int:
    """Fail every encoding, as a producer whose writer refused would."""
    return -1


def _describe(_source: int, code: MutableSequence[int], output: int | None, capacity: int) -> int:
    """Report the diagnostic number and message of the failed encoding."""
    code[0] = PARSE_ERROR_NUMBER
    if output is None or capacity == 0:
        return len(MESSAGE)
    ctypes.memmove(output, MESSAGE, len(MESSAGE))
    return len(MESSAGE)


class _FailingStructure:
    """A structure whose capsule is a real C record that cannot encode."""

    def __init__(self, version: int) -> None:
        self._offsets = (ctypes.c_uint32 * 1)(0)
        self._source = _Source(
            abi_version=version,
            coordinates=ctypes.addressof(self._offsets),
            atoms=ctypes.addressof(self._offsets),
            bonds=ctypes.addressof(self._offsets),
            residue_atom_start=ctypes.addressof(self._offsets),
            residue_atom_start_len=1,
            chain_residue_start=ctypes.addressof(self._offsets),
            chain_residue_start_len=1,
            model_chain_start=ctypes.addressof(self._offsets),
            model_chain_start_len=1,
            select=_Select(_refuse_select),
            encode_bcif=_Encode(_refuse_encode),
            describe_failure=_Describe(_describe),
        )
        new = ctypes.pythonapi.PyCapsule_New
        new.restype = ctypes.py_object
        new.argtypes = (ctypes.c_void_p, ctypes.c_char_p, ctypes.c_void_p)
        self._capsule = new(ctypes.addressof(self._source), CAPSULE_NAME, None)

    def _molframe_source_v2(self) -> object:
        return self._capsule

    def _molframe_secondary_structure(self) -> bytes:
        return b""

    def _molframe_altlocs(self) -> np.ndarray[Any, np.dtype[np.uint32]]:
        return np.empty(0, dtype=np.uint32)


class NativeFailureTests(unittest.TestCase):
    """Drive the real binding with a producer that refuses to encode."""

    def test_the_producers_exception_and_code_surface_instead_of_none(self) -> None:
        with self.assertRaises(molframe.errors.ParseError) as raised:
            molgfx.Scene(_FailingStructure(FAILURE_ABI_VERSION))
        self.assertEqual(raised.exception.code, f"MOLFRAME-E{PARSE_ERROR_NUMBER}")
        self.assertIn(MESSAGE.decode(), str(raised.exception))

    def test_a_base_version_producer_raises_a_generic_error_never_none(self) -> None:
        with self.assertRaises(molframe.errors.MolframeValueError) as raised:
            molgfx.Scene(_FailingStructure(BASE_ABI_VERSION))
        self.assertIn("BCIF encoding failed", str(raised.exception))


if __name__ == "__main__":
    unittest.main()
