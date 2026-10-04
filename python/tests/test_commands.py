"""The authoring command language through Python: sessions, typed commands, errors."""

import json
import unittest
from typing import cast

import molframe

import molgfx
from molgfx import Command, CommandError, CommandResult, Session

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


def _structure() -> molframe.Structure:
    return molframe.read(MMCIF, name="two.cif")


def _operations(result: CommandResult) -> list[str]:
    if result.patch is None:
        return []
    return [operation["op"] for operation in json.loads(result.patch.to_json())["operations"]]


class SessionTests(unittest.TestCase):
    """A session's own scene, its revisions, and the commands that drive them."""

    def test_a_session_over_a_structure_owns_a_live_scene(self) -> None:
        session = Session(_structure())
        result = session.execute("show cartoon, protein")
        self.assertEqual(_operations(result), ["add_representation"])
        self.assertEqual(session.scene.revision, result.revision)
        self.assertEqual(session.structures, {"s1": 1})

    def test_a_session_over_a_scene_shares_that_scene(self) -> None:
        scene = molgfx.Scene(_structure())
        session = Session(scene)
        session.execute("show spacefill as lig, resname HEM")
        self.assertIs(session.scene, scene)
        self.assertEqual(len(json.loads(scene.to_json())["representations"]), 1)

    def test_assembly_unit_cell_is_scene_state_and_undoable(self) -> None:
        session = Session(_structure())
        command = (
            'assembly {"structures":[1],"instances":[],"unit_cell":'
            '{"lengths":[10.0,11.0,12.0],"angles_degrees":[90.0,90.0,90.0],'
            '"origin":[0.0,0.0,0.0]}}'
        )
        session.execute(command)
        self.assertEqual(json.loads(session.scene.to_json())["assembly"]["structures"], [1])
        session.execute("undo")
        self.assertIsNone(json.loads(session.scene.to_json())["assembly"])

    def test_removing_an_assembly_restores_its_exact_state_on_undo(self) -> None:
        session = Session(_structure())
        assembly = {
            "structures": [1],
            "instances": [],
            "unit_cell": {
                "lengths": [10.0, 11.0, 12.0],
                "angles_degrees": [80.0, 95.0, 105.0],
                "origin": [1.0, 2.0, 3.0],
            },
        }
        session.execute("assembly " + json.dumps(assembly))
        clear = Command.from_json(json.dumps({"command": "assembly", "assembly": None}))
        session.execute(Command.parse(str(clear)))
        self.assertIsNone(json.loads(session.scene.to_json())["assembly"])
        session.undo()
        self.assertEqual(json.loads(session.scene.to_json())["assembly"], assembly)
        session.redo()
        self.assertIsNone(json.loads(session.scene.to_json())["assembly"])

    def test_show_is_idempotent(self) -> None:
        session = Session(_structure())
        session.execute("show cartoon, protein")
        again = session.execute("show cartoon, protein")
        self.assertEqual(_operations(again), ["set_visibility"])
        self.assertEqual(len(session.layers), 1)

    def test_a_redefined_selection_moves_what_uses_it(self) -> None:
        session = Session(_structure())
        session.execute("select site, chain A; show spacefill as site, $site")
        result = session.execute("select site, chain B")
        self.assertEqual(_operations(result), ["set_representation_target"])
        self.assertIn("$site", session.explain_layer("site"))

    def test_undo_and_redo(self) -> None:
        session = Session(_structure())
        session.execute("show cartoon, protein")
        self.assertTrue(session.can_undo)
        session.undo()
        self.assertEqual(session.layers, {})
        self.assertTrue(session.can_redo)
        session.redo()
        self.assertEqual(list(session.layers), ["cartoon"])

    def test_errors_are_typed_located_and_suggested(self) -> None:
        session = Session(_structure())
        session.execute("select pocket, resname HEM")
        with self.assertRaises(CommandError) as raised:
            session.execute("show cartoon, $pockt")
        [error] = raised.exception.errors
        self.assertEqual(error["kind"], "unknown_symbol")
        self.assertEqual(error["suggestion"], "pocket")
        self.assertEqual(error["span"], (14, 20))
        self.assertIn("^^^^^^", str(raised.exception))

    def test_a_failing_program_changes_nothing(self) -> None:
        session = Session(_structure())
        revision = session.scene.revision
        with self.assertRaises(CommandError):
            session.execute("show cartoon, protein; hide @missing")
        self.assertEqual(session.scene.revision, revision)
        self.assertEqual(session.layers, {})

    def test_typed_commands_run_like_text(self) -> None:
        session = Session(_structure())
        command = Command.show("ball_and_stick", "resname HEM", layer="lig", radius=0.3)
        self.assertEqual(str(command), "show ball_and_stick radius=0.3 as lig, resname HEM")
        session.execute(command)
        session.execute([Command.color("red", "@lig"), Command.opacity(0.5, "lig")])
        self.assertEqual(Command.parse(str(command)), command)
        with self.assertRaises(ValueError):
            Command.show("spacefill", "all", style="rocket")

    def test_every_derived_colour_scheme_applies_to_a_live_layer(self) -> None:
        # Secondary structure, chain and metric columns all come from the
        # molframe structure the adapter imports; a column the adapter failed
        # to fill used to validate as an all-NaN property and reject the whole
        # program.
        session = Session(_structure())
        session.execute("show cartoon, protein")
        for scheme in ("secondary_structure", "chain", "molecule_type", "b_factor"):
            with self.subTest(scheme=scheme):
                session.execute(f"color {scheme}, @cartoon")

    def test_completions_follow_the_cursor(self) -> None:
        session = Session(_structure())
        session.execute("select pocket, resname HEM")
        texts = [text for text, _kind, _detail in session.completions("show cartoon, $po")]
        self.assertIn("$pocket", texts)

    def test_a_session_round_trips_through_json(self) -> None:
        session = Session(_structure())
        session.execute("select site, resname HEM; show spacefill as lig, $site")
        restored = Session.from_json(session.scene, session.to_json())
        self.assertEqual(restored.selections, {"site": "resname HEM"})
        self.assertEqual(list(restored.layers), ["lig"])

    def test_the_vocabulary_lists_forms_and_verbs(self) -> None:
        vocabulary = molgfx.vocabulary()
        # `vocabulary` hands back parsed JSON, so its shape is whatever the
        # engine emitted; this test is where the two entries it promises are
        # pinned down.
        forms = cast("list[str]", vocabulary["forms"])
        verbs = cast("list[tuple[str, ...]]", vocabulary["verbs"])
        self.assertIn("cartoon", forms)
        self.assertIn("show", [verb[0] for verb in verbs])


class SnapshotTests(unittest.TestCase):
    """Named scene captures restore actual authored state and participate in history."""

    def test_snapshot_restore_and_undo_recover_styles_overlays_and_camera(self) -> None:
        session = Session(_structure())
        session.execute('show spacefill as atoms; color red, chain A; label "saved", chain A')
        session.scene.set_camera(session.scene.frame(molgfx.sel.all()))
        session.snapshot_save("initial")
        initial = self.authored(session)
        session.execute('hide @atoms; color blue, chain A; label "changed", chain B')
        session.scene.set_camera(None)
        changed = self.authored(session)
        revision = session.scene.revision
        result = session.snapshot_restore("initial")
        self.assertEqual(_operations(result), ["restore_snapshot"])
        self.assertEqual(session.scene.revision, revision + 1)
        self.assertEqual(self.authored(session), initial)
        session.undo()
        self.assertEqual(self.authored(session), changed)
        session.redo()
        self.assertEqual(self.authored(session), initial)

    def test_snapshot_names_survive_session_json_and_remove_is_undoable(self) -> None:
        session = Session(_structure())
        session.snapshot_save("initial")
        saved = session.to_json()
        session.snapshot_remove("initial")
        with self.assertRaises(CommandError):
            session.snapshot_restore("initial")
        session.undo()
        self.assertEqual(session.to_json(), saved)
        restored = Session.from_json(session.scene, saved)
        restored.snapshot_restore("initial")

    def test_unknown_snapshot_and_invalid_program_leave_both_states_unchanged(self) -> None:
        session = Session(_structure())
        scene_before = session.scene.to_json()
        names_before = session.to_json()
        with self.assertRaises(CommandError):
            session.execute("snapshot save first; snapshot restore absent")
        self.assertEqual(session.scene.to_json(), scene_before)
        self.assertEqual(session.to_json(), names_before)

    @staticmethod
    def authored(session: Session) -> dict[str, object]:
        """Ignore the deliberately monotonic revision when comparing authored state."""
        state = cast("dict[str, object]", json.loads(session.scene.to_json()))
        state.pop("revision")
        return state


if __name__ == "__main__":
    unittest.main()
