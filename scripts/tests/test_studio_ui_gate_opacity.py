"""G2 opacity: a `State=Disabled` variant carries 0.5 on every visible paint (fill, stroke, text, glyph), every other variant of a
component set on `02 Core` carries 1.0. `kit/mod.rs` half_color halves each paint's alpha, so it is a paint opacity.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_opacity
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_fixtures import GREY, make_doc, node  # noqa: E402
from studio_ui_gate_opacity import check_opacity  # noqa: E402
from test_studio_ui_gate_registry import add_set  # noqa: E402

SET = "02 Core/Button/Icon"


def paint(opacity=1.0, visible=True):
    return {"kind": {"Solid": GREY}, "opacity": opacity, "visible": visible, "blend": "Normal"}


class OpacityTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()
        add_set(self.doc, 20, "Button/Icon", {"State": ["Default", "Disabled"]})  # variants 21 (Default) and 22 (Disabled)

    def put(self, nid, **extra):
        self.doc["nodes"][str(nid)].update(extra)

    def check(self):
        return check_opacity(self.doc)

    def test_half_on_every_paint_of_a_disabled_variant_and_one_elsewhere_passes(self):
        self.put(21, fills=[paint(1.0)], stroke={"paints": [paint(1.0)], "weight": 1.0})
        self.put(22, fills=[paint(0.5)], stroke={"paints": [paint(0.5)], "weight": 1.0})
        fails, counts = self.check()
        self.assertEqual(fails, [])
        self.assertEqual((counts["variants"], counts["disabled"], counts["paints"]), (4, 1, 4))  # 4: the fixture's Divider/Hairline and its plain Divider/Plain too

    def test_a_disabled_variant_with_a_full_opacity_fill_fails(self):
        self.put(22, fills=[paint(1.0)])
        self.assertEqual(self.check()[0], [f"{SET}/State=Disabled: fills/0 has opacity 1, a Disabled variant carries 0.5 on every visible paint"])

    def test_a_default_variant_with_a_dimmed_fill_fails(self):
        self.put(21, fills=[paint(0.3)])
        self.assertEqual(self.check()[0], [f"{SET}/State=Default: fills/0 has opacity 0.3, a non-Disabled variant carries 1 on every visible paint"])

    def test_a_text_fill_and_a_nested_glyph_are_read_like_the_gate_reads_them(self):
        self.put(22, kind={"Component": {}}, children=[23, 24])
        self.put(21, fills=[])
        self.doc["nodes"]["23"] = node(23, "Label", {"Text": {"content": "x", "style": {"fills": [paint(1.0)]}}}, parent=22)
        self.doc["nodes"]["24"] = node(24, "Icon", {"Instance": {"frame": {}, "component": 9}}, parent=22, children=[25])
        self.doc["nodes"]["25"] = node(25, "Glyph", "Vector", parent=24, fills=[paint(1.0)])
        fails = self.check()[0]
        self.assertEqual(sorted(f.split(": ")[0] for f in fails), [f"{SET}/State=Disabled/Icon/Glyph", f"{SET}/State=Disabled/Label"])

    def test_an_invisible_paint_is_not_read(self):
        self.put(22, fills=[paint(1.0, visible=False), paint(0.5)])
        self.assertEqual(self.check()[0], [])

    def test_only_the_exact_state_part_makes_a_variant_disabled(self):
        self.doc["nodes"]["22"]["name"] = "State=DisabledX"
        self.put(22, fills=[paint(0.5)])
        self.assertEqual(len(self.check()[0]), 1)
        self.doc["nodes"]["22"]["name"] = "Size=Md, State=Disabled"
        self.assertEqual(self.check()[0], [])

    def test_a_node_opacity_on_a_default_variant_fails(self):
        """The kit never dims a node: the engine applies node opacity, so a 0.5 node would double a paint's own alpha."""
        self.put(21, opacity=0.5)
        self.assertEqual(self.check()[0], [f"{SET}/State=Default: has node opacity 0.5, the kit never dims a node (Disabled is half alpha per paint)"])

    def test_a_node_opacity_on_a_disabled_variant_is_a_double_dim(self):
        self.put(22, fills=[paint(0.5)], opacity=0.5)
        self.assertEqual(len(self.check()[0]), 1)

    def test_a_node_opacity_on_a_nested_label_fails(self):
        self.put(21, kind={"Component": {}}, children=[23], fills=[])
        self.doc["nodes"]["23"] = node(23, "Label", {"Text": {"content": "x", "style": {"fills": [paint(1.0)]}}}, parent=21, opacity=0.3)
        self.assertEqual(self.check()[0], [f"{SET}/State=Default/Label: has node opacity 0.3, the kit never dims a node (Disabled is half alpha per paint)"])

    def test_a_full_node_opacity_passes(self):
        self.put(21, opacity=1.0)
        self.assertEqual(self.check()[0], [])

    def test_sets_on_other_pages_are_not_read(self):
        self.doc["nodes"]["2"]["children"].append(30)
        self.doc["nodes"]["30"] = node(30, "Other", {"ComponentSet": {}}, parent=2, children=[31])
        self.doc["nodes"]["31"] = node(31, "State=Default", {"Component": {}}, parent=30, fills=[paint(0.2)])
        self.assertEqual(self.check()[0], [])


if __name__ == "__main__":
    unittest.main()
