"""Frame packing for the chunk transport: every accepted input yields the same float32 bytes."""

import unittest
from array import array
from itertools import chain

from molgfx.viewer._transport import frame_bytes

ROWS = [[0.5, -1.25, 3.0], [100.0, 0.1, -7.5], [1.0e-3, 2.0, 4.0]]
EXPECTED = array("f", chain.from_iterable(ROWS)).tobytes()


class FrameBytesTests(unittest.TestCase):
    """``frame_bytes`` packs one ``(atoms, 3)`` frame as native float32 triples."""

    def test_a_float32_buffer_is_copied_unchanged(self) -> None:
        self.assertEqual(frame_bytes(array("f", chain.from_iterable(ROWS))), EXPECTED)

    def test_nested_lists_pack_to_float32_bytes(self) -> None:
        self.assertEqual(frame_bytes(ROWS), EXPECTED)

    def test_nested_tuples_and_generators_pack_to_the_same_bytes(self) -> None:
        self.assertEqual(frame_bytes(tuple(tuple(row) for row in ROWS)), EXPECTED)
        self.assertEqual(frame_bytes(iter(ROWS)), EXPECTED)

    def test_float64_rows_narrow_to_the_same_bytes_as_a_float32_buffer(self) -> None:
        rows = [array("d", row) for row in ROWS]
        self.assertEqual(frame_bytes(rows), EXPECTED)

    def test_a_flat_float64_buffer_is_refused_with_a_type_error(self) -> None:
        with self.assertRaises(TypeError):
            frame_bytes(array("d", chain.from_iterable(ROWS)))

    def test_a_non_iterable_frame_is_refused_with_a_type_error(self) -> None:
        with self.assertRaises(TypeError):
            frame_bytes(3.5)

    def test_flat_lists_of_numbers_are_refused_with_a_type_error(self) -> None:
        with self.assertRaises(TypeError):
            frame_bytes([1.0, 2.0, 3.0])

    def test_every_input_form_yields_byte_identical_output(self) -> None:
        forms = (
            array("f", chain.from_iterable(ROWS)),
            ROWS,
            [array("d", row) for row in ROWS],
            [tuple(row) for row in ROWS],
        )
        self.assertEqual({frame_bytes(form) for form in forms}, {EXPECTED})


if __name__ == "__main__":
    unittest.main()
