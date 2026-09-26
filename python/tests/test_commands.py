"""The authoring command language through Python: sessions, typed commands,
errors and the Workbench transport, all without a browser."""

import json
import unittest

import molframe
import molgfx
from molgfx import Command, CommandError, Session


MMCIF = b"""data_two
_entry.id two
loop_
_entity.id
_entity.type
1 polymer
2 non-polymer
loop_
_entity_poly.entity_id
_entity_poly.type
1 'polypeptide(L)'
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
_atom_site.auth_seq_id
_atom_site.auth_comp_id
_atom_site.auth_asym_id
_atom_site.auth_atom_id
ATOM 1 N N GLY A 1 1 0.0 0.0 0.0 1 GLY A N
ATOM 2 C CA GLY A 1 1 1.4 0.0 0.0 1 GLY A CA
ATOM 3 N N GLY A 1 2 3.0 0.5 0.0 2 GLY A N
ATOM 4 C CA GLY A 1 2 4.4 0.5 0.0 2 GLY A CA
ATOM 5 N N GLY B 1 1 0.0 5.0 0.0 1 GLY B N
ATOM 6 C CA GLY B 1 1 1.4 5.0 0.0 1 GLY B CA
ATOM 7 N N GLY B 1 2 3.0 5.5 0.0 2 GLY B N
ATOM 8 C CA GLY B 1 2 4.4 5.5 0.0 2 GLY B CA
HETATM 9 FE FE HEM C 2 . 2.0 2.5 0.0 101 HEM C FE
"""


def structure():
    return molframe.read(MMCIF, name="two.cif")


def operations(result):
    if result.patch is None:
        return []
    return [operation["op"] for operation in json.loads(result.patch.to_json())["operations"]]


class SessionTests(unittest.TestCase):
    def test_a_session_over_a_structure_owns_a_live_scene(self):
        session = Session(structure())
        result = session.execute("show cartoon, protein")
        self.assertEqual(operations(result), ["add_representation"])
        self.assertEqual(session.scene.revision, result.revision)
        self.assertEqual(session.structures, {"s1": 1})

    def test_a_session_over_a_scene_shares_that_scene(self):
        scene = molgfx.Scene(structure())
        session = Session(scene)
        session.execute("show spacefill as lig, resname HEM")
        self.assertIs(session.scene, scene)
        self.assertEqual(len(json.loads(scene.to_json())["representations"]), 1)

    def test_show_is_idempotent(self):
        session = Session(structure())
        session.execute("show cartoon, protein")
        again = session.execute("show cartoon, protein")
        self.assertEqual(operations(again), ["set_visibility"])
        self.assertEqual(len(session.layers), 1)

    def test_a_redefined_selection_moves_what_uses_it(self):
        session = Session(structure())
        session.execute("select site, chain A; show spacefill as site, $site")
        result = session.execute("select site, chain B")
        self.assertEqual(operations(result), ["set_representation_target"])
        self.assertIn("$site", session.explain_layer("site"))

    def test_undo_and_redo(self):
        session = Session(structure())
        session.execute("show cartoon, protein")
        self.assertTrue(session.can_undo)
        session.undo()
        self.assertEqual(session.layers, {})
        self.assertTrue(session.can_redo)
        session.redo()
        self.assertEqual(list(session.layers), ["cartoon"])

    def test_errors_are_typed_located_and_suggested(self):
        session = Session(structure())
        session.execute("select pocket, resname HEM")
        with self.assertRaises(CommandError) as raised:
            session.execute("show cartoon, $pockt")
        [error] = raised.exception.errors
        self.assertEqual(error["kind"], "unknown_symbol")
        self.assertEqual(error["suggestion"], "pocket")
        self.assertEqual(error["span"], (14, 20))
        self.assertIn("^^^^^^", str(raised.exception))

    def test_a_failing_program_changes_nothing(self):
        session = Session(structure())
        revision = session.scene.revision
        with self.assertRaises(CommandError):
            session.execute("show cartoon, protein; hide @missing")
        self.assertEqual(session.scene.revision, revision)
        self.assertEqual(session.layers, {})

    def test_typed_commands_run_like_text(self):
        session = Session(structure())
        command = Command.show("ball_and_stick", "resname HEM", layer="lig", radius=0.3)
        self.assertEqual(str(command), "show ball_and_stick radius=0.3 as lig, resname HEM")
        session.execute(command)
        session.execute([Command.color("red", "@lig"), Command.opacity(0.5, "lig")])
        self.assertEqual(Command.parse(str(command)), command)
        with self.assertRaises(ValueError):
            Command.show("spacefill", "all", style="rocket")

    def test_completions_follow_the_cursor(self):
        session = Session(structure())
        session.execute("select pocket, resname HEM")
        texts = [text for text, _kind, _detail in session.completions("show cartoon, $po")]
        self.assertIn("$pocket", texts)

    def test_a_session_round_trips_through_json(self):
        session = Session(structure())
        session.execute("select site, resname HEM; show spacefill as lig, $site")
        restored = Session.from_json(session.scene, session.to_json())
        self.assertEqual(restored.selections, {"site": "resname HEM"})
        self.assertEqual(list(restored.layers), ["lig"])

    def test_the_vocabulary_lists_forms_and_verbs(self):
        vocabulary = molgfx.vocabulary()
        self.assertIn("cartoon", vocabulary["forms"])
        self.assertIn("show", [verb[0] for verb in vocabulary["verbs"]])


class WorkbenchTransportTests(unittest.TestCase):
    """What the Workbench exchanges with its page, driven without a browser."""

    def test_a_page_command_reaches_the_canvas_as_a_patch(self):
        from molgfx.viewer import Workbench

        bench = Workbench(structure())
        bench.command_request = {"type": "execute", "id": "1", "text": "show cartoon, protein"}
        self.assertTrue(bench.command_reply["ok"], bench.command_reply)
        self.assertEqual(bench.command_reply["id"], "1")
        self.assertEqual(bench.history, ["show cartoon, protein"])
        self.assertEqual(bench.patch_sequence, 1)
        self.assertIn("add_representation", bench.scene_patch)

    def test_a_page_error_is_reported_not_raised(self):
        from molgfx.viewer import Workbench

        bench = Workbench(structure())
        bench.command_request = {"type": "execute", "id": "2", "text": "shwo cartoon, all"}
        reply = bench.command_reply
        self.assertFalse(reply["ok"])
        self.assertEqual(reply["errors"][0]["suggestion"], "show")
        self.assertEqual(bench.patch_sequence, 0)

    def test_a_page_completion_request_is_answered(self):
        from molgfx.viewer import Workbench

        bench = Workbench(structure())
        bench.command_request = {"type": "complete", "id": "3", "text": "show car", "cursor": 8}
        texts = [item["text"] for item in bench.command_reply["items"]]
        self.assertIn("cartoon", texts)

    def test_kernel_commands_share_the_page_history(self):
        from molgfx.viewer import Workbench

        bench = Workbench(structure())
        bench.execute("select site, resname HEM")
        bench.execute("show spacefill, $site")
        self.assertEqual(bench.history, ["select site, resname HEM", "show spacefill, $site"])
        self.assertEqual(bench.patch_sequence, 1)


if __name__ == "__main__":
    unittest.main()
