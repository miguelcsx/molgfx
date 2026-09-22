import json
import unittest

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


def structure():
    return molframe.read(MMCIF, name="one.cif")


class SceneSemanticsTests(unittest.TestCase):
    def test_transaction_commits_one_revision(self):
        scene = molgfx.Scene(structure())
        opacity = molgfx.visual.parameter(0.5, name="opacity_scale")
        style = molgfx.visual.style(
            color=molgfx.visual.color((30, 120, 220)),
            opacity=opacity,
        )
        representation = scene.add(
            molgfx.rep.spacefill(target=molgfx.sel.all()).visual(style)
        )
        revision = scene.revision

        with scene.transaction():
            scene.set_parameter(representation, opacity, 0.8)
            scene.set_opacity(representation, 0.9)

        self.assertEqual(scene.revision, revision + 1)
        self.assertIn("opacity_scale", scene.to_json())

    def test_invalid_transaction_rolls_back_every_operation(self):
        scene = molgfx.Scene(structure())
        representation = scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))
        revision = scene.revision
        before = scene.to_json()

        with self.assertRaises(TypeError):
            with scene.transaction():
                scene.set_opacity(representation, 0.5)
                scene.set_opacity(2**63, 0.3)

        self.assertEqual(scene.revision, revision)
        self.assertEqual(scene.to_json(), before)

    def test_parameter_types_preserve_their_values(self):
        vector = molgfx.visual.vector_parameter(
            (1.0, 0.0, 0.0),
            name="direction",
        )
        color = molgfx.visual.color_parameter((10, 20, 30), name="tint")

        self.assertEqual(vector.default, (1.0, 0.0, 0.0))
        self.assertEqual(color.default, (10, 20, 30))

    def test_scene_returns_typed_semantic_ids(self):
        scene = molgfx.Scene(structure())
        representation = scene.add(molgfx.rep.spacefill(target=molgfx.sel.all()))

        self.assertIsInstance(scene.structure_id, molgfx.StructureId)
        self.assertIsInstance(representation, molgfx.RepresentationId)
        self.assertEqual(int(scene.structure_id), 1)
        self.assertEqual(int(representation), 1)

    def test_all_scientific_items_use_the_common_add_path(self):
        scene = molgfx.Scene(structure())
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
            molgfx.density.volume(source=source, dimensions=(8, 8, 8))
        )
        label = scene.add(
            molgfx.annotation.label(anchor=origin, text="active site")
        )
        distance = scene.add(molgfx.measurement.distance(origin, x_axis))
        angle = scene.add(molgfx.measurement.angle(origin, x_axis, y_axis))
        dihedral = scene.add(
            molgfx.measurement.dihedral(origin, x_axis, y_axis, z_axis)
        )
        explicit = scene.add(
            molgfx.interaction.explicit(
                kind="contact", first=origin, second=x_axis
            )
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
        self.assertIsInstance(explicit, molgfx.ScientificInteractionId)
        self.assertIsInstance(trajectory, molgfx.TrajectoryId)
        spec = json.loads(scene.to_json())
        self.assertEqual(len(spec["volumes"]), 1)
        self.assertEqual(len(spec["annotations"]), 1)
        self.assertEqual(len(spec["measurements"]), 3)
        self.assertEqual(len(spec["scientific_interactions"]), 1)
        self.assertEqual(len(spec["trajectories"]), 1)

    def test_a_bound_trajectory_pair_reaches_the_renderer(self):
        scene = molgfx.Scene(structure())
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

    def test_an_unbound_trajectory_source_stays_unresolved(self):
        scene = molgfx.Scene(structure())
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

    def test_property_binding_is_owned_by_its_structure(self):
        scene = molgfx.Scene(structure())
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
        representation = scene.add(
            molgfx.rep.spacefill(target=molgfx.sel.all()).visual(style)
        )

        self.assertIsInstance(representation, molgfx.RepresentationId)
        self.assertIn("confidence", scene.to_json())

    def test_interaction_channels_serialize_and_validate_names(self):
        scene = molgfx.Scene(structure())
        scene.set_interaction(channel="selected", target=molgfx.sel.all())
        scene.set_interaction(
            channel="custom", name="candidate", target=molgfx.sel.water()
        )

        spec = json.loads(scene.to_json())
        self.assertIsNotNone(spec["selected"])
        self.assertIn("candidate", spec["custom_interactions"])
        with self.assertRaises(ValueError):
            scene.set_interaction(channel="custom", name="")

    def test_browser_sources_are_encoded_lazily_and_stable(self):
        scene = molgfx.Scene(structure())
        first = scene._browser_sources()
        second = scene._browser_sources()

        self.assertEqual(len(first), 1)
        self.assertEqual(first, second)
        self.assertGreater(len(first[0][2]), 0)


    def test_detected_interactions_are_not_exposed(self):
        self.assertFalse(hasattr(molgfx.interaction, "detected"))
        self.assertFalse(hasattr(molgfx.ScientificInteraction, "detected"))

    def test_a_covalent_disulfide_is_not_an_authorable_interaction(self):
        with self.assertRaises(TypeError):
            molgfx.interaction.explicit(
                kind="disulfide",
                first=molgfx.annotation.world((0.0, 0.0, 0.0)),
                second=molgfx.annotation.world((1.0, 0.0, 0.0)),
            )

    def test_every_remaining_interaction_kind_is_authorable(self):
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
                molgfx.ScientificInteraction,
            )


class ViewerTransportTests(unittest.TestCase):
    def test_direct_mutations_publish_exact_incremental_patches(self):
        from molgfx.viewer import Viewer

        scene = molgfx.Scene(structure())
        viewer = Viewer(scene)
        representation = scene.add(molgfx.rep.points(target=molgfx.sel.all()))

        self.assertEqual(viewer.patch_sequence, 1)
        patch = json.loads(viewer.scene_patch)
        self.assertEqual(patch["base_revision"], 0)
        self.assertEqual(patch["operations"][0]["op"], "add_representation")

        with scene.transaction():
            scene.set_visible(representation, False)
            scene.set_opacity(representation, 0.4)

        self.assertEqual(viewer.patch_sequence, 2)
        patch = json.loads(viewer.scene_patch)
        self.assertEqual(len(patch["operations"]), 2)
        self.assertEqual(
            [operation["op"] for operation in patch["operations"]],
            ["set_visibility", "set_opacity"],
        )


if __name__ == "__main__":
    unittest.main()
