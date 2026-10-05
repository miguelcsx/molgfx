"""Scene semantics through Python: typed ids, patches, properties and interactions."""

import json
import unittest
import weakref
from typing import Any

import molframe

import molgfx

MMCIF = b"""data_one
_entry.id one
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
_atom_site.occupancy
_atom_site.B_iso_or_equiv
ATOM 1 N N ALA A 1 1 11.104 13.207 9.274 1.00 20.00
ATOM 2 C CA ALA A 1 1 12.560 13.318 9.111 1.00 20.00
"""


def _structure() -> molframe.Structure:
    return molframe.read(MMCIF, name="one.cif")


ASSEMBLY = b"""data_demo
loop_
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.auth_seq_id
_atom_site.auth_asym_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
1 C CA GLY A 1 1 A 1 0 0
2 C CA GLY B 1 1 B 0 2 0
loop_
_pdbx_struct_oper_list.id
_pdbx_struct_oper_list.matrix[1][1]
_pdbx_struct_oper_list.matrix[1][2]
_pdbx_struct_oper_list.matrix[1][3]
_pdbx_struct_oper_list.vector[1]
_pdbx_struct_oper_list.matrix[2][1]
_pdbx_struct_oper_list.matrix[2][2]
_pdbx_struct_oper_list.matrix[2][3]
_pdbx_struct_oper_list.vector[2]
_pdbx_struct_oper_list.matrix[3][1]
_pdbx_struct_oper_list.matrix[3][2]
_pdbx_struct_oper_list.matrix[3][3]
_pdbx_struct_oper_list.vector[3]
I 1 0 0 0 0 1 0 0 0 0 1 0
S 1 0 0 30 0 1 0 0 0 0 1 0
_pdbx_struct_assembly.id 1
loop_
_pdbx_struct_assembly_gen.assembly_id
_pdbx_struct_assembly_gen.oper_expression
_pdbx_struct_assembly_gen.asym_id_list
1 I A,B
1 S A
"""


class SceneSemanticsTests(unittest.TestCase):
    """What the scene hands back, and what it refuses."""

    def test_cartoon_shape_controls_survive_scene_serialization(self) -> None:
        """The shared scene contract retains authored cross-section controls."""
        scene = molgfx.Scene(_structure())
        scene.add(
            molgfx.rep.cartoon(
                target="all",
                aspect_ratio=3.0,
                arrow_factor=2.0,
                direction_wedges=True,
                gaps="dashed",
                helix_profile="rounded",
                nucleic_profile="square",
            )
        )
        text = scene.to_json()
        self.assertIn('"aspect_ratio":3.0', text)
        self.assertIn('"arrow_factor":2.0', text)
        self.assertIn('"direction_wedges":true', text)
        self.assertIn('"gaps":"dashed"', text)
        self.assertIn('"helix_profile":"rounded"', text)
        self.assertIn('"nucleic_profile":"square"', text)
        for aspect, arrow in ((0.0, 1.0), (5.0, -1.0), (float("nan"), 1.0)):
            with self.assertRaises(molgfx.SpecError):
                scene.add(molgfx.rep.cartoon(target="all", aspect_ratio=aspect, arrow_factor=arrow))

    def test_an_unoriented_guide_reports_a_render_error_and_the_renderer_recovers(self) -> None:
        """Incomplete backbone data cannot turn an authored wedge into silent background."""
        invalid = molgfx.Scene(_structure())
        invalid.add(molgfx.rep.cartoon(target="all", direction_wedges=True))
        renderer = molgfx.Renderer(profile=molgfx.profile.interactive())
        with self.assertRaisesRegex(molgfx.MolgfxError, "polymer guide .* has no source direction"):
            renderer.render_image(invalid, size=(64, 64))
        valid = molgfx.Scene(_structure())
        valid.add(molgfx.rep.spacefill(target="all"))
        self.assertEqual(renderer.render_image(valid, size=(64, 64)).width, 64)

    def test_glycan_direction_wedges_are_rejected_atomically(self) -> None:
        """A polymer direction control cannot silently disappear in glycan mode."""
        scene = molgfx.Scene(_structure())
        before = scene.to_json()
        with self.assertRaises(molgfx.SpecError):
            scene.add(molgfx.rep.cartoon(target="all", style="glycan", direction_wedges=True))
        self.assertEqual(scene.to_json(), before)

    def test_transaction_commits_one_revision(self) -> None:
        scene = molgfx.Scene(_structure())
        opacity = molgfx.visual.parameter(0.5, name="opacity_scale")
        style = molgfx.visual.style(
            color=molgfx.visual.color((30, 120, 220)),
            opacity=opacity,
        )
        representation = scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()).visual(style))
        revision = scene.revision

        with scene.transaction():
            scene.set_parameter(representation, opacity, 0.8)
            scene.set_opacity(representation, 0.9)

        self.assertEqual(scene.revision, revision + 1)
        self.assertIn("opacity_scale", scene.to_json())

    def test_invalid_transaction_rolls_back_every_operation(self) -> None:
        scene = molgfx.Scene(_structure())
        representation = scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
        revision = scene.revision
        before = scene.to_json()
        # An id no representation holds: the stub promises `RepresentationId`,
        # and the rollback is what has to survive the binding refusing it.
        absent: Any = 2**63

        with self.assertRaises(TypeError), scene.transaction():
            scene.set_opacity(representation, 0.5)
            scene.set_opacity(absent, 0.3)

        self.assertEqual(scene.revision, revision)
        self.assertEqual(scene.to_json(), before)

    def test_parameter_types_preserve_their_values(self) -> None:
        vector = molgfx.visual.vector_parameter(
            (1.0, 0.0, 0.0),
            name="direction",
        )
        color = molgfx.visual.color_parameter((10, 20, 30), name="tint")

        self.assertEqual(vector.default, (1.0, 0.0, 0.0))
        self.assertEqual(color.default, (10, 20, 30))

    def test_scene_returns_typed_semantic_ids(self) -> None:
        scene = molgfx.Scene(_structure())
        representation = scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))

        self.assertIsInstance(scene.structure_id, molgfx.StructureId)
        self.assertIsInstance(representation, molgfx.RepresentationId)
        self.assertEqual(int(scene.structure_id), 1)
        self.assertEqual(int(representation), 1)

    def test_all_overlay_items_use_the_common_add_path(self) -> None:
        scene = molgfx.Scene(_structure())
        origin = molgfx.annotation.world((0.0, 0.0, 0.0))
        x_axis = molgfx.annotation.world((1.0, 0.0, 0.0))
        y_axis = molgfx.annotation.world((1.0, 1.0, 0.0))
        z_axis = molgfx.annotation.world((1.0, 1.0, 1.0))
        source = molgfx.data.source(
            "density-sha256",
            uri="https://example.invalid/density.ccp4",
            format="ccp4",
        )

        volume = scene.add(
            molgfx.density.volume(source=source, dimensions=(8, 8, 8)).isosurface(1.0)
        )
        label = scene.add(molgfx.annotation.label(anchor=origin, text="active site"))
        distance = scene.add(molgfx.measurement.distance(origin, x_axis))
        angle = scene.add(molgfx.measurement.angle(origin, x_axis, y_axis))
        dihedral = scene.add(molgfx.measurement.dihedral(origin, x_axis, y_axis, z_axis))
        explicit = scene.add(
            molgfx.interaction.explicit(kind="contact", first=origin, second=x_axis)
        )
        trajectory = scene.add(
            molgfx.trajectory.bind(
                structure=scene.structure_id,
                source=molgfx.data.source("trajectory-sha256"),
                frame_count=10,
                time_step=0.5,
                time_unit="ps",
            )
        )

        self.assertIsInstance(volume, molgfx.VolumeId)
        self.assertIsInstance(label, molgfx.AnnotationId)
        self.assertIsInstance(distance, molgfx.MeasurementId)
        self.assertIsInstance(angle, molgfx.MeasurementId)
        self.assertIsInstance(dihedral, molgfx.MeasurementId)
        self.assertIsInstance(explicit, molgfx.InteractionId)
        self.assertIsInstance(trajectory, molgfx.TrajectoryId)
        spec = json.loads(scene.to_json())
        self.assertEqual(len(spec["volumes"]), 1)
        self.assertEqual(len(spec["annotations"]), 1)
        self.assertEqual(len(spec["measurements"]), 3)
        self.assertEqual(len(spec["interactions"]), 1)
        self.assertEqual(len(spec["trajectories"]), 1)

    def test_a_bound_trajectory_pair_reaches_the_renderer(self) -> None:
        scene = molgfx.Scene(_structure())
        source = molgfx.data.source("trajectory-sha256")
        scene.add(
            molgfx.trajectory.bind(
                structure=scene.structure_id,
                source=source,
                frame_count=2,
            )
        )

        start = molgfx.trajectory.frame(0, 0.0, ((11.104, 13.207, 9.274), (12.56, 13.318, 9.111)))
        end = molgfx.trajectory.frame(1, 1.0, ((11.904, 13.207, 9.274), (12.56, 13.318, 9.111)))
        scene.bind_trajectory(source_hash="trajectory-sha256", start=start, end=end)

        # Advancing inside the resident interval samples the pair already held,
        # so it uploads no coordinates.
        scene.set_trajectory_time(structure=scene.structure_id, seconds=0.5)
        # The resident interval is [0, 1]; a sample outside it is refused at
        # the core seam, which surfaces as the general engine error.
        with self.assertRaises(molgfx.MolgfxError):
            scene.set_trajectory_time(structure=scene.structure_id, seconds=5.0)

    def test_an_unbound_trajectory_source_stays_unresolved(self) -> None:
        scene = molgfx.Scene(_structure())
        scene.add(
            molgfx.trajectory.bind(
                structure=scene.structure_id,
                source=molgfx.data.source("declared"),
                frame_count=2,
            )
        )
        start = molgfx.trajectory.frame(0, 0.0, ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)))
        end = molgfx.trajectory.frame(1, 1.0, ((1.0, 0.0, 0.0), (0.0, 0.0, 0.0)))
        scene.bind_trajectory(source_hash="other", start=start, end=end)

        # The descriptor remains in the specification; a binding for a
        # different source does not satisfy it.
        self.assertEqual(len(json.loads(scene.to_json())["trajectories"]), 1)

    def test_property_binding_is_owned_by_its_structure(self) -> None:
        scene = molgfx.Scene(_structure())
        prop = scene.bind_property(
            structure=scene.structure_id,
            name="confidence",
            source_hash="confidence-sha256",
            values=(0.25, 0.75),
            units="fraction",
            domain=(0.0, 1.0),
        )
        style = molgfx.visual.style(
            color=molgfx.visual.ramp(
                molgfx.visual.property(prop),
                palette="viridis",
                domain=(0.0, 1.0),
            )
        )
        representation = scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()).visual(style))

        self.assertIsInstance(representation, molgfx.RepresentationId)
        self.assertIn("confidence", scene.to_json())

    def test_interaction_channels_serialize_and_validate_names(self) -> None:
        scene = molgfx.Scene(_structure())
        scene.set_interaction(channel="selected", target=molgfx.sel.all())
        scene.set_interaction(channel="custom", name="candidate", target=molgfx.sel.water())

        spec = json.loads(scene.to_json())
        self.assertIsNotNone(spec["selected"])
        self.assertIn("candidate", spec["custom_interactions"])
        with self.assertRaises(ValueError):
            scene.set_interaction(channel="custom", name="")

    def test_browser_sources_are_encoded_lazily_and_stable(self) -> None:
        scene = molgfx.Scene(_structure())
        first = scene.browser_sources()
        second = scene.browser_sources()

        self.assertEqual(len(first), 1)
        self.assertEqual(first, second)
        self.assertGreater(len(first[0][2]), 0)

    def test_detected_interactions_are_not_exposed(self) -> None:
        self.assertFalse(hasattr(molgfx.interaction, "detected"))
        self.assertFalse(hasattr(molgfx.Interaction, "detected"))

    def test_a_covalent_disulfide_is_not_an_authorable_interaction(self) -> None:
        # A disulfide is a bond in the structure, not an interaction a caller
        # may name, so the stub cannot offer the spelling and the binding has
        # to refuse it.
        covalent: Any = "disulfide"
        with self.assertRaises(TypeError):
            molgfx.interaction.explicit(
                kind=covalent,
                first=molgfx.annotation.world((0.0, 0.0, 0.0)),
                second=molgfx.annotation.world((1.0, 0.0, 0.0)),
            )

    def test_every_remaining_interaction_kind_is_authorable(self) -> None:
        first = molgfx.annotation.world((0.0, 0.0, 0.0))
        second = molgfx.annotation.world((1.0, 0.0, 0.0))
        for kind in (
            "hydrogen_bond",
            "salt_bridge",
            "pi_stacking",
            "cation_pi",
            "hydrophobic",
            "metal_coordination",
            "contact",
        ):
            self.assertIsInstance(
                molgfx.interaction.explicit(kind=kind, first=first, second=second),
                molgfx.Interaction,
            )

    def test_auto_draws_the_default_forms_and_returns_typed_ids(self) -> None:
        scene = molgfx.Scene(_structure())
        ids = scene.auto()
        self.assertTrue(ids)
        for identifier in ids:
            self.assertIsInstance(identifier, molgfx.RepresentationId)
        spec = json.loads(scene.to_json())
        self.assertEqual(len(spec["representations"]), len(ids))

    def test_pocket_composes_six_editable_forms_and_focuses_the_subject(self) -> None:
        scene = molgfx.Scene(_structure())
        style = molgfx.PocketStyle(near=3.0, mid=8.0).with_opacity(pocket=0.5)
        ids = scene.pocket("name CA", style=style)
        self.assertEqual(len(ids), 6)
        for identifier in ids:
            self.assertIsInstance(identifier, molgfx.RepresentationId)
        self.assertEqual(len(json.loads(scene.to_json())["representations"]), 6)
        self.assertEqual(style.pocket_opacity, 0.5)
        self.assertEqual(style.near, 3.0)

    def test_pocket_command_text_and_builder_agree(self) -> None:
        scene = molgfx.Scene(_structure())
        session = molgfx.Session(scene)
        session.execute("pocket near=3, name CA")
        self.assertEqual(len(session.layers), 6)
        built = molgfx.Command.pocket("name CA", near=3.0)
        self.assertEqual(built.verb, "pocket")
        self.assertEqual(json.loads(built.to_json())["near"], 3.0)
        with self.assertRaises(ValueError):
            molgfx.Command.pocket("name CA", near=-1.0)

    def test_pocket_rejects_an_empty_focus_and_unordered_distances(self) -> None:
        scene = molgfx.Scene(_structure())
        with self.assertRaises(molgfx.SpecError):
            scene.pocket("resname NOPE")
        with self.assertRaises(molgfx.SpecError):
            scene.pocket("name CA", style=molgfx.PocketStyle(near=9.0, mid=3.0))
        with self.assertRaises(molgfx.SpecError):
            scene.pocket("name CA", style=molgfx.PocketStyle().with_opacity(solvent=2.0))
        self.assertEqual(json.loads(scene.to_json())["representations"], {})

    def test_camera_paths_sample_validate_and_reject_bad_keyframes(self) -> None:
        def camera(x: float) -> molgfx.Camera:
            return molgfx.Camera(position=(x, 0.0, 10.0), target=(0.0, 0.0, 0.0))

        path = molgfx.CameraPath([(0.0, camera(0.0)), (2.0, camera(8.0))], easing="linear")
        self.assertEqual((len(path), path.range), (2, (0.0, 2.0)))
        # Paths orbit the target: the midpoint is between the ends, not their mean.
        middle = path.sample(1.0)
        assert middle is not None, "a time inside the path samples a camera"
        self.assertTrue(1.0 < middle.position[0] < 7.0)
        before = path.sample(-3.0)
        assert before is not None, "a time before the path clamps to the first keyframe"
        self.assertEqual(before.position[0], 0.0)
        self.assertIsNone(path.sample(float("nan")))
        # An easing the path cannot express is not one the stub may offer.
        unknown_easing: Any = "bounce"
        with self.assertRaises(ValueError):
            molgfx.CameraPath([(0.0, camera(0.0))])
        with self.assertRaises(ValueError):
            molgfx.CameraPath([(1.0, camera(0.0)), (1.0, camera(1.0))])
        with self.assertRaises(ValueError):
            molgfx.CameraPath([(0.0, camera(0.0)), (1.0, camera(1.0))], easing=unknown_easing)

    def test_confidence_and_rainbow_are_metrics_with_aliases(self) -> None:
        scene = molgfx.Scene(_structure())
        for name in ("plddt", "confidence", "rainbow", "sequence_position", "b_factor"):
            spec = molgfx.color.metric(name)
            scene.add(molgfx.rep.cartoon(target=molgfx.sel.all(), color=spec))
        unknown_metric: Any = "no-such-metric"
        with self.assertRaises(ValueError):
            molgfx.color.metric(unknown_metric)
        self.assertIn("plddt", molgfx.color.ramp_names())

    def test_an_ensemble_overlays_structures_by_weight(self) -> None:
        scene = molgfx.Scene(_structure())
        second = scene.add_structure(_structure())
        self.assertNotEqual(second, scene.structure_id)
        ids = scene.ensemble(
            [
                (scene.structure_id, 3.0, (200, 40, 40)),
                (second, 1.0, (40, 40, 200)),
            ]
        )
        self.assertEqual(len(ids), 2)
        representations = json.loads(scene.to_json())["representations"]
        opacity = {
            entry["common"]["structure"]: entry["common"]["opacity"]
            for entry in representations.values()
        }
        self.assertEqual(opacity[int(scene.structure_id)], 1.0)
        self.assertAlmostEqual(opacity[int(second)], 0.55 / 3.0, places=5)
        with self.assertRaises(molgfx.SpecError):
            scene.ensemble([(scene.structure_id, 0.0, (1, 2, 3))])
        with self.assertRaises(molgfx.SpecError):
            scene.ensemble(
                [(scene.structure_id, 1.0, (1, 2, 3)), (scene.structure_id, 1.0, (1, 2, 3))]
            )

    def test_a_difference_view_colours_by_the_bound_property(self) -> None:
        scene = molgfx.Scene(_structure())
        prop = scene.bind_property(
            structure=scene.structure_id,
            name="delta",
            source_hash="delta-sha256",
            values=(0.0, 2.0),
        )
        style = molgfx.DifferenceStyle(thresholds=(0.0, 2.0), domain=(0.0, 2.0), palette="viridis")
        self.assertEqual(style.thresholds, (0.0, 2.0))
        representation = scene.difference(prop, "all", style=style)
        self.assertIsInstance(representation, molgfx.RepresentationId)
        with self.assertRaises(molgfx.SpecError):
            scene.difference(prop, "all", style=molgfx.DifferenceStyle(thresholds=(2.0, 1.0)))
        with self.assertRaises(molgfx.SpecError):
            scene.difference(prop, "all", style=molgfx.DifferenceStyle(palette="no-such-palette"))

    def test_a_placed_copy_is_a_structure_of_its_own_with_its_own_forms(self) -> None:
        scene = molgfx.Scene(_structure())
        shift = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 25.0, 0, 0, 1]
        copy = scene.place(shift)
        self.assertIsInstance(copy, molgfx.StructureId)
        self.assertNotEqual(copy, scene.structure_id)
        scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()).on(copy))
        spec = json.loads(scene.to_json())
        self.assertEqual(len(spec["structures"]), 2)
        placed = [s for s in spec["structures"].values() if "placement" in s]
        self.assertEqual(len(placed), 1)
        self.assertEqual(placed[0]["placement"][12], 25.0)
        with self.assertRaises(ValueError):
            scene.place([1.0] * 15)
        with self.assertRaises(molgfx.SpecError):
            scene.place([0.0] * 16)

    @unittest.skipUnless(
        hasattr(molframe, "crystal") and hasattr(molframe.crystal, "assembly"),
        "needs a molframe that exposes biological assemblies",
    )
    def test_an_assembly_becomes_placed_copies_each_drawn_with_its_own_chains(self) -> None:
        assembled = molframe.read(ASSEMBLY, name="assembly.cif")
        scene = molgfx.Scene(assembled)
        copies = scene.assembly(molframe.crystal.assembly(assembled, "1"))
        self.assertEqual([copy.chains for copy in copies], [["A", "B"], ["A"]])
        self.assertEqual(copies[1].selection, "label_chain A")
        for copy in copies:
            scene.add(molgfx.rep.spacefill(target=copy.selection).on(copy.structure))
        spec = json.loads(scene.to_json())
        self.assertEqual(len(spec["structures"]), 3)
        self.assertEqual(len(spec["representations"]), 2)
        shifted = [s for s in spec["structures"].values() if "placement" in s]
        self.assertEqual(sorted(s["placement"][12] for s in shifted), [0.0, 30.0])
        with self.assertRaises(AttributeError):
            scene.assembly([molgfx.Scene])  # no matrix or chains attribute


class PatchStreamTests(unittest.TestCase):
    """What a host that mirrors a scene receives, without any host present."""

    def test_direct_mutations_publish_exact_incremental_patches(self) -> None:
        class Host:
            def __init__(self) -> None:
                self.received: list[dict[str, Any]] = []

            def on_patch(self, patch_json: str) -> None:
                self.received.append(json.loads(patch_json))

        scene = molgfx.Scene(_structure())
        host = Host()
        scene.subscribe(weakref.WeakMethod(host.on_patch))
        representation = scene.add(molgfx.rep.points(target=molgfx.sel.all()))

        self.assertEqual(len(host.received), 1)
        patch = host.received[0]
        self.assertEqual(patch["base_revision"], 0)
        self.assertEqual(patch["operations"][0]["op"], "add_representation")

        with scene.transaction():
            scene.set_visible(representation, visible=False)
            scene.set_opacity(representation, 0.4)

        self.assertEqual(len(host.received), 2)
        self.assertEqual(
            [operation["op"] for operation in host.received[1]["operations"]],
            ["set_visibility", "set_opacity"],
        )


if __name__ == "__main__":
    unittest.main()
