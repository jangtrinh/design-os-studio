"""A mark that covers the whole searched interior in both images has no foreground box (the interior is one flat colour), so the box check
reads "ok" for any two sizes and the surface pick is the mark. The W1a review's H1 and H2 synthetics (a 33 px mark against 35 px, a 34 px mark
against 38 px, both in a 40 px crop) passed. `ambiguous_surface` therefore also fails closed when an image has no foreground box and the surface
colour does not own nearly every pixel (a lead under `FLAT_LEAD`): a crop of a plain surface, with nothing drawn in it, is still clear.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_flat_interior
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import crop_metrics as cm  # noqa: E402
from test_studio_ui_crop_metrics import BG, FILL, GREY, WHITE, grid  # noqa: E402


def glyph(side, offset, w=40, h=40, colour=GREY):
    """A `side` px square mark at (offset, offset) on the page colour: the review's H1 and H2 crops are 40 x 40, searched from 4 to 36."""
    return grid(w, h, BG, g=(offset, offset, side, side, colour))


class FlatInteriorTest(unittest.TestCase):
    def pair(self, render, prod, **entry):
        return cm.measure_pixels((render, 40, 40), (prod, 40, 40), entry)

    def test_h1_a_33_px_mark_against_35_px_fails_closed(self):
        """Lead 0.36 / 0.53, both interiors flat: the boxes are None and equal, so this passed."""
        m = self.pair(glyph(33, 3), glyph(35, 2))
        self.assertIsNone(m["box_render"])
        self.assertIsNone(m["box_prod"])
        self.assertTrue(m["ambiguous_render"] and m["ambiguous_prod"] and m["ambiguous_fails"] and m["bad"], m)

    def test_h2_a_34_px_mark_against_38_px_fails_closed(self):
        m = self.pair(glyph(34, 3), glyph(38, 1))
        self.assertTrue(m["ambiguous_fails"] and m["bad"], m)

    def test_one_image_with_a_flat_mark_interior_is_enough(self):
        m = self.pair(glyph(10, 15), glyph(35, 2))
        self.assertFalse(m["ambiguous_render"])
        self.assertTrue(m["ambiguous_prod"] and m["ambiguous_fails"] and m["bad"], m)

    def test_a_plain_surface_with_nothing_drawn_is_clear(self):
        """No mark at all: the lead is 1.0, the same surface in both images is a legitimate pass."""
        flat = grid(40, 40, FILL)
        m = self.pair(flat, flat)
        self.assertFalse(m["ambiguous_render"] or m["ambiguous_prod"] or m["ambiguous_fails"] or m["bad"], m)

    def test_a_surface_with_a_few_stray_pixels_is_still_clear(self):
        """Anti-aliased corner pixels (1 per 100) leave the lead above the flat threshold."""
        px = grid(40, 40, FILL, corner=(0, 0, 4, 4, BG))  # 16 of 1600 pixels
        self.assertFalse(cm.ambiguous_surface(px, 40, 40))

    def test_the_lead_threshold_is_a_named_constant_just_under_one(self):
        self.assertEqual(cm.FLAT_LEAD, 0.98)

    def test_a_pending_entry_is_exempt_like_every_other_check(self):
        m = self.pair(glyph(33, 3), glyph(35, 2), fg_pending="#1427")
        self.assertFalse(m["bad"])
        enforced = cm.measure_pixels((glyph(33, 3), 40, 40), (glyph(35, 2), 40, 40), {"fg_pending": "#1427"}, pending_open=False)
        self.assertTrue(enforced["ambiguous_fails"] and enforced["bad"], enforced)

    def test_a_mark_with_a_foreground_box_keeps_its_verdict(self):
        """The foreground is a glyph in the middle of a control: boxes exist, nothing changes for the 26 real rows."""
        m = self.pair(glyph(12, 14, colour=WHITE), glyph(12, 14, colour=WHITE))
        self.assertIsNotNone(m["box_render"])
        self.assertFalse(m["ambiguous_fails"] or m["bad"], m)


if __name__ == "__main__":
    unittest.main()
