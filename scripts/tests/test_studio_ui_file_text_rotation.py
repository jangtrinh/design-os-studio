"""`T(..., rot=-90)` is a text turned a quarter turn counter-clockwise about its top-left corner, as egui's `TextShape::with_angle(-FRAC_PI_2)` paints the vertical ruler's labels (canvas/rulers.rs
`rotated_text`). The lowerer writes it as the REST `relativeTransform` (designos-fig rest_import.rs:1222 reads it, `size` stays the unrotated box). The corner is the node's position in its parent, so
a rotated text needs an absolutely placed slot (`abs`), in a frame with or without auto layout. A text without `rot` carries no transform key. Synthetic plan only (no MCP).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_text_rotation
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402


def lower(text, parent_at=(100, 40), at=(12, 30), **kw):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    kid = ir.T("Label", "5000", "Body / Medium", "color/line", abs=(at[0], at[1], 24, 12), **kw)
    parent = ir.F("Strip", [kid], w=20, h=300, sx="fixed", sy="fixed", abs=None)
    return low.lower(parent, parent_at[0], parent_at[1], ptype="COMPONENT")["children"][0]


class RotationTest(unittest.TestCase):
    def test_a_quarter_turn_counter_clockwise_is_the_rest_matrix_with_the_corner_in_the_parent(self):
        t = lower("x", rot=-90)
        (a, c, tx), (b, d, ty) = t["relativeTransform"]
        self.assertEqual((round(a, 6), round(b, 6), round(c, 6), round(d, 6)), (0.0, -1.0, 1.0, 0.0))  # local x runs up the page, local y runs right
        self.assertEqual((tx, ty), (12, 30))

    def test_the_corner_is_relative_to_the_parent_not_to_the_page(self):
        near, far = lower("x", parent_at=(0, 0), rot=-90), lower("x", parent_at=(500, 700), rot=-90)
        self.assertEqual(near["relativeTransform"], far["relativeTransform"])

    def test_the_size_is_the_unrotated_box(self):
        t = lower("x", rot=-90)
        self.assertEqual((t["absoluteBoundingBox"]["width"], t["absoluteBoundingBox"]["height"]), (24, 12))

    def test_a_text_without_rot_has_no_transform_key(self):
        self.assertNotIn("relativeTransform", lower("x"))

    def test_only_the_quarter_turn_is_known(self):
        for bad in (90, 45, "-90", True, 0):
            with self.assertRaises(ValueError, msg=bad):
                ir.T("Label", "x", "Body / Medium", "color/line", rot=bad)

    def test_a_rotated_text_that_is_not_absolutely_placed_fails_loudly(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        row = ir.F("Row", [ir.T("Label", "x", "Body / Medium", "color/line", rot=-90)], dir="H", w=40, h=40, sx="fixed", sy="fixed")
        with self.assertRaises(ValueError):
            low.lower(row, 0, 0, ptype="COMPONENT")

    def test_an_absolutely_placed_rotated_text_in_an_auto_layout_frame_keeps_its_corner(self):
        low = Lowerer(PLAN, STYLES, FakeIcons())
        row = ir.F("Row", [ir.T("Label", "x", "Body / Medium", "color/line", rot=-90, abs=(6, 30, 24, 12))], dir="H", w=40, h=40, sx="fixed", sy="fixed")
        t = low.lower(row, 100, 40, ptype="COMPONENT")["children"][0]
        self.assertEqual((t["relativeTransform"][0][2], t["relativeTransform"][1][2]), (6, 30))
        self.assertEqual(t["layoutPositioning"], "ABSOLUTE")


if __name__ == "__main__":
    unittest.main()
