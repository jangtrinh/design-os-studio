"""G2 (bound): every fill, stroke, radius, gap, padding and effect colour on pages 01-03 is bound to a Semantic variable.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_bound
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import check_studio_ui_file as gate  # noqa: E402
from studio_ui_gate_fixtures import CORNERS, GREY, frame_kind, make_doc, solid, with_node  # noqa: E402

B = "01 Foundations/Swatch"


class BoundBase(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()

    def bound(self, allow=()):
        return gate.check_bound(self.doc, list(allow))[0]

    def bindings(self, **keys):
        b = self.doc["nodes"]["4"]["variables"]["bindings"]
        b.update(keys)
        return b


class CoreTest(BoundBase):
    def test_clean_document_passes(self):
        fails, per_page = gate.check_bound(self.doc, [])
        self.assertEqual(fails, [])
        self.assertEqual(per_page["01 Foundations"]["bound"], 5)
        self.assertEqual(per_page["02 Core"]["nodes"], 4)  # the fixture's component set and its variant, a plain main and its instance: no bindable property
        self.assertEqual(per_page["03 Studio"]["nodes"], 0)  # empty pages pass while the overall selection is not empty

    def test_unbound_fill_fails(self):
        del self.bindings()["fills/0"]
        self.assertEqual(self.bound(), [f"{B}: fills/0 is not bound"])

    def test_unbound_radius_gap_padding_and_stroke_fail(self):
        n = with_node(self.doc, kind=frame_kind(item_spacing=8.0, padding=[0.0, 12.0, 0.0, 0.0]), stroke={"paints": [solid()], "weight": 1.0})
        n["variables"]["bindings"] = {"fills/0": "s.bg"}
        keys = {f.split(": ")[1].split(" ")[0] for f in self.bound()}
        self.assertEqual(keys, {*CORNERS, "itemSpacing", "paddingRight", "strokes/0", "strokeWeight"})

    def test_counter_axis_spacing_must_be_bound(self):
        with_node(self.doc, kind=frame_kind(counter_spacing=6.0))
        self.assertIn(f"{B}: counterAxisSpacing is not bound", self.bound())

    def test_text_fill_in_style_must_be_bound(self):
        with_node(self.doc, kind={"Text": {"content": "x", "style": {"fills": [solid()]}}}, fills=[])
        self.assertEqual(self.bound(), [])
        del self.bindings()["fills/0"]
        self.assertEqual(self.bound(), [f"{B}: fills/0 is not bound"])

    def test_binding_to_a_primitive_fails(self):
        self.bindings(**{"fills/0": "p.grey"})
        self.assertTrue(any("non-Semantic" in f for f in self.bound()))

    def test_allow_list_waives_one_property(self):
        del self.bindings()["fills/0"]
        grey = {"r": 23, "g": 23, "b": 23, "a": 255}
        self.assertEqual(self.bound([{"path": B, "property": "fills/0", "reason": "test", "value": grey}]), [])

    def test_empty_selection_is_an_error(self):
        self.doc["nodes"]["2"]["children"] = []
        self.doc["nodes"]["3"]["children"] = []
        with self.assertRaises(gate.GateInputError):
            self.bound()


class ScopeOfTheCheckTest(BoundBase):
    def test_pages_outside_01_to_03_are_not_gated(self):
        self.doc["nodes"]["1"]["children"] = [11]
        self.doc["nodes"]["11"] = {"id": 11, "name": "Loose", "kind": "Rectangle", "parent": 1, "children": [], "fills": [solid()]}
        self.assertEqual(self.bound(), [])

    def test_gated_pages_02_and_03_are_checked(self):
        self.doc["nodes"]["3"]["children"] = [12]
        self.doc["nodes"]["12"] = {"id": 12, "name": "Loose", "kind": "Rectangle", "parent": 3, "children": [], "fills": [solid()]}
        self.assertEqual(self.bound(), ["02 Core/Loose: fills/0 is not bound"])

    def test_a_hidden_unbound_fill_is_not_a_finding(self):
        with_node(self.doc, fills=[solid(visible=False)], corner_radii=[0.0] * 4)
        del self.bindings()["fills/0"]
        self.assertEqual(self.bound(), [])


class InstanceTest(BoundBase):
    def test_instance_gap_and_padding_must_be_bound(self):
        n = with_node(self.doc, kind=frame_kind("Instance", item_spacing=8.0, padding=[12.0, 0.0, 12.0, 0.0]))
        n["variables"]["bindings"] = {"fills/0": "s.bg", **{k: "s.fact" for k in CORNERS}}
        self.assertEqual({f.split(": ")[1].split(" ")[0] for f in self.bound()}, {"itemSpacing", "paddingTop", "paddingBottom"})

    def test_bound_instance_passes(self):
        n = with_node(self.doc, kind=frame_kind("Instance", item_spacing=8.0))
        n["variables"]["bindings"]["itemSpacing"] = "s.inline"
        self.assertEqual(self.bound(), [])


class BlindSpotTest(BoundBase):
    def test_unbound_side_stroke_weight_fails(self):
        with_node(self.doc, stroke={"paints": [solid()], "weight": 1.0, "side_weights": [0.0, 0.0, 3.0, 0.0]})
        self.bindings(**{"strokes/0": "s.bg", "strokeWeight": "s.fact"})
        self.assertIn(f"{B}: strokeBottomWeight is not bound", self.bound())

    def test_bound_side_stroke_weight_passes(self):
        with_node(self.doc, stroke={"paints": [solid()], "weight": 1.0, "side_weights": [0.0, 0.0, 4.0, 0.0]})
        self.bindings(**{"strokes/0": "s.bg", "strokeBottomWeight": "s.fact"})
        self.assertEqual(self.bound(), [])

    def test_literal_shadow_colour_fails(self):
        with_node(self.doc, effects=[{"DropShadow": {"color": GREY, "offset": {"x": 0, "y": 2}, "radius": 4.0, "spread": 0.0, "visible": True}}])
        self.assertIn(f"{B}: effects/0/color is not bound", self.bound())
        self.bindings(**{"effects/0/color": "s.bg"})
        self.assertEqual(self.bound(), [])

    def test_fill_bound_to_a_number_variable_fails(self):
        self.bindings(**{"fills/0": "s.fact"})
        self.assertTrue(any("kind Number" in f and "fills/0" in f for f in self.bound()), self.bound())

    def test_stored_value_must_equal_the_bound_variable(self):
        n = with_node(self.doc, kind=frame_kind(item_spacing=99.0))
        n["variables"]["bindings"]["itemSpacing"] = "s.inline"
        self.assertTrue(any("itemSpacing" in f and "99" in f and "8" in f for f in self.bound()), self.bound())

    def test_a_fill_that_differs_from_its_variable_fails(self):
        with_node(self.doc, fills=[solid(dict(GREY, r=0.5))])
        self.assertTrue(any("fills/0" in f and "differs" in f for f in self.bound()), self.bound())

    def test_uniform_corner_radius_binding_counts_as_unbound(self):
        """0.7.5 ignores a uniform `cornerRadius` binding (render, codegen and has-hardcoded skip it): only the four corner keys bind."""
        b = self.doc["nodes"]["4"]["variables"]["bindings"]
        for k in CORNERS:
            del b[k]
        b["cornerRadius"] = "s.fact"
        self.assertEqual(self.bound(), [f"{B}: {k} is not bound" for k in CORNERS])

    def test_the_four_corner_keys_bind_the_radius(self):
        self.assertEqual(self.bound(), [])


class GraphTest(BoundBase):
    def test_dangling_child_is_an_input_error_not_a_traceback(self):
        self.doc["nodes"]["2"]["children"] = [4, 999999]
        with self.assertRaises(gate.GateInputError):
            self.bound()

    def test_a_child_cycle_is_an_input_error(self):
        self.doc["nodes"]["4"]["children"] = [2]
        with self.assertRaises(gate.GateInputError):
            self.bound()


if __name__ == "__main__":
    unittest.main()
