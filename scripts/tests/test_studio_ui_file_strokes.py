"""Per-side strokes on frames and stroked open paths, as the 0.7.5 importer reads them.

Probed on designos-mcp 0.7.5 (a REST file rendered at 4x): `individualStrokeWeights` {top, right, bottom, left} draws only the sides with a weight (bottom 2 alone; top 3 with left 1), and an
open `VECTOR` with `strokes`, `strokeWeight`, `strokeCap` and `strokeJoin` and no fill is drawn as a stroked line (round caps and joins, or butt caps). So `F(sw=[top, right, bottom, left])`
writes the four weights and binds the side keys the gate reads (`strokeTopWeight` ...), and `V(.., None, stroke=, sw=, cap=, join=)` is a stroked path. Synthetic plan.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_strokes
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402

WHY = "a raw stroke width in the Rust"


def lower(node):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    return low, low.lower(node, 0, 0, ptype="COMPONENT")


def keys(low):
    return sorted((i["key"], i["var"]) for i in low.intents)


class SideStrokeTest(unittest.TestCase):
    def tab(self, sw, **kw):
        return ir.F("Tab", fill="color/bg", stroke="color/line", sw=sw, w=40, h=24, sx="fixed", sy="fixed", **kw)

    def test_a_bottom_only_stroke_writes_the_four_weights_and_binds_the_bottom_key_alone(self):
        low, rest = lower(self.tab([None, None, "size/border-width", None]))
        self.assertEqual(rest["individualStrokeWeights"], {"top": 0, "right": 0, "bottom": 1.0, "left": 0})
        self.assertEqual(rest["strokeWeight"], 1.0)
        self.assertEqual([k for k in keys(low) if "eight" in k[0]], [("strokeBottomWeight", "size/border-width")])
        self.assertIn(("strokes/0", "color/line"), keys(low))

    def test_every_chosen_side_binds_its_own_key(self):
        low, rest = lower(self.tab(["size/border-width", None, None, "size/border-width"]))
        self.assertEqual(rest["individualStrokeWeights"], {"top": 1.0, "right": 0, "bottom": 0, "left": 1.0})
        self.assertEqual([k[0] for k in keys(low) if "eight" in k[0]], ["strokeLeftWeight", "strokeTopWeight"])

    def test_a_literal_side_is_an_allow_list_entry_on_its_key_and_binds_nothing(self):
        low, rest = lower(self.tab([None, None, ir.lit(2, WHY), None]))
        self.assertEqual(rest["individualStrokeWeights"]["bottom"], 2.0)
        self.assertEqual([k for k in keys(low) if "eight" in k[0]], [])
        self.assertEqual([(a["property"], a["value"], a["reason"]) for a in low.allow], [("strokeBottomWeight", 2.0, WHY)])

    def test_a_uniform_weight_is_unchanged(self):
        low, rest = lower(self.tab("size/border-width"))
        self.assertNotIn("individualStrokeWeights", rest)
        self.assertEqual([k for k in keys(low) if "eight" in k[0]], [("strokeWeight", "size/border-width")])

    def test_a_side_list_that_is_not_four_entries_or_names_no_side_is_refused(self):
        for bad in (["size/border-width"], [None, None, None, None], [None, None, ir.lit(0, WHY), None], [None, None, 3, None], "size/border-width" and [None, 1.0, None, None]):
            with self.assertRaises(ValueError, msg=bad):
                self.tab(bad)

    def test_a_side_list_needs_a_stroke_colour(self):
        with self.assertRaises(ValueError):
            ir.F("Tab", sw=[None, None, "size/border-width", None])


class StrokedPathTest(unittest.TestCase):
    def chevron(self, **kw):
        return ir.V("Chevron", "M0 4 L4.5 0 L9 4", 9, 5, None, stroke="color/line", sw=ir.lit(1.5, WHY), **kw)

    def test_a_stroked_path_has_no_fill_and_a_centred_stroke_with_its_cap_and_join(self):
        low, rest = lower(self.chevron(cap="round", join="round"))
        self.assertEqual((rest["fills"], rest["strokeAlign"], rest["strokeCap"], rest["strokeJoin"], rest["strokeWeight"]), ([], "CENTER", "ROUND", "ROUND", 1.5))
        self.assertEqual(rest["fillGeometry"], [{"path": "M0 4 L4.5 0 L9 4", "windingRule": "NONZERO"}])

    def test_the_stroke_binds_its_colour_and_records_its_literal_weight(self):
        low, _ = lower(self.chevron())
        self.assertEqual(keys(low), [("strokes/0", "color/line")])
        self.assertEqual([(a["property"], a["value"]) for a in low.allow], [("strokeWeight", 1.5)])

    def test_the_defaults_are_butt_caps_and_a_miter_join(self):
        _, rest = lower(self.chevron())
        self.assertEqual((rest["strokeCap"], rest["strokeJoin"]), ("NONE", "MITER"))

    def test_an_explicit_alignment_is_kept(self):
        self.assertEqual(lower(self.chevron(sa="inside"))[1]["strokeAlign"], "INSIDE")

    def test_an_unknown_cap_or_join_is_refused(self):
        for kw in ({"cap": "flat"}, {"join": "sharp"}):
            with self.assertRaises(ValueError, msg=kw):
                self.chevron(**kw)

    def test_a_filled_path_is_unchanged(self):
        _, rest = lower(ir.V("Dot", "M0 0 L4 0 L4 4 Z", 4, 4, "color/bg"))
        self.assertEqual((len(rest["fills"]), rest.get("strokes")), (1, None))


if __name__ == "__main__":
    unittest.main()
