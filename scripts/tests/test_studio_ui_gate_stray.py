"""G2 stray: a variable binding on a property of G2's families that the node does not set fails (designos-mcp 0.7.5 draws it at the variable's value). Size bindings and the committed pair are
not flagged.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_stray
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import check_studio_ui_file as gate  # noqa: E402
from studio_ui_gate_fixtures import REPO, frame_kind, make_doc, node, solid, with_node  # noqa: E402
from studio_ui_gate_stray import check_stray  # noqa: E402

B = "01 Foundations/Swatch"
WHY = "is a binding on a property the node does not set: the engine draws it at the variable's value"


class StrayTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()

    def bind(self, **keys):
        self.doc["nodes"]["4"]["variables"]["bindings"].update(keys)

    def stray(self):
        return check_stray(self.doc)[0]

    def test_a_clean_document_has_none(self):
        self.assertEqual(check_stray(self.doc), ([], 5))

    def test_a_side_binding_on_a_side_with_no_weight_fails(self):
        with_node(self.doc, stroke={"paints": [solid()], "weight": 1.0, "side_weights": [0.0, 0.0, 4.0, 0.0]})
        self.bind(**{"strokes/0": "s.bg", "strokeBottomWeight": "s.fact"})
        self.assertEqual(self.stray(), [])
        self.bind(strokeTopWeight="s.fact")
        self.assertEqual(self.stray(), [f"{B}: strokeTopWeight {WHY}"])

    def test_each_family_is_read(self):
        """One stray key per family on a node that sets none of them: a second fill, a stroke, a uniform weight, a side, a corner, a padding, a gap, a counter-axis gap, an effect color."""
        with_node(self.doc, kind=frame_kind(), corner_radii=[0.0] * 4)
        self.doc["nodes"]["4"]["variables"]["bindings"].clear()
        self.bind(**{"fills/0": "s.bg"})
        strays = {"fills/1": "s.bg", "fills/0/stops/0": "s.bg", "strokes/0": "s.bg", "strokeWeight": "s.fact", "strokeLeftWeight": "s.fact", "topLeftRadius": "s.fact",
                  "paddingRight": "s.inline", "itemSpacing": "s.inline", "counterAxisSpacing": "s.inline", "effects/0/color": "s.bg"}
        for key, var in strays.items():
            with self.subTest(key):
                self.bind(**{key: var})
                self.assertEqual(self.stray(), [f"{B}: {key} {WHY}"])
                del self.doc["nodes"]["4"]["variables"]["bindings"][key]

    def test_counter_axis_spacing_and_effect_color_pass_when_set_and_fail_when_unset(self):
        with_node(self.doc, kind=frame_kind(counter_spacing=6.0), effects=[{"DropShadow": {"visible": True, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.5}}}])
        self.bind(**{"counterAxisSpacing": "s.inline", "effects/0/color": "s.bg"})
        self.assertEqual(self.stray(), [])
        self.bind(**{"effects/1/color": "s.bg", "effects/0/radius": "s.fact"})
        self.assertEqual(self.stray(), [f"{B}: effects/1/color {WHY}"])

    def test_a_size_binding_is_outside_the_families(self):
        """16 `width` and `height` bindings sit on the committed file, none of them on a property G2 reads."""
        self.bind(width="s.fact", height="s.fact")
        self.assertEqual(self.stray(), [])

    def test_a_uniform_corner_radius_binding_is_outside_the_families(self):
        self.bind(cornerRadius="s.fact")
        self.assertEqual(self.stray(), [])

    def test_pages_after_03_are_not_read(self):
        self.doc["nodes"]["10"] = node(10, "04 Screens", "Page", children=[11])
        self.doc["nodes"]["11"] = node(11, "Screen", parent=10, variables={"modes": {}, "bindings": {"strokeBottomWeight": "s.fact"}})
        self.doc["pages"].append(10)
        self.assertEqual(self.stray(), [])

    def test_every_stray_is_reported_with_its_path(self):
        self.bind(strokeBottomWeight="s.fact", paddingLeft="s.inline")
        self.assertEqual(sorted(self.stray()), [f"{B}: paddingLeft {WHY}", f"{B}: strokeBottomWeight {WHY}"])

    def test_a_dangling_child_is_an_input_error(self):
        self.doc["nodes"]["2"]["children"] = [4, 999999]
        with self.assertRaises(gate.GateInputError):
            self.stray()


class CommittedFileTest(unittest.TestCase):
    def test_the_committed_file_has_no_stray_binding(self):
        doc = json.loads((REPO / "design" / "studio-ui.designos").read_text(encoding="utf-8"))
        fails, read = check_stray(doc)
        self.assertEqual(fails, [])
        self.assertGreater(read, 9000, "the committed file's bindings on pages 01-03 were counted")


if __name__ == "__main__":
    unittest.main()
