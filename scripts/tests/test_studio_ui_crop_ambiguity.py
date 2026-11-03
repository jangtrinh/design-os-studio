"""The compare fails closed when the surface it picked is ambiguous. The surface is the colour most pixels of a crop have, which is right when the
control's fill or the page dominates. It is wrong when the glyph or label is the majority (a filled mark, a label in a tight crop) or when two
colours split the crop, and a wrong pick can pass a wrong drawing or fail a right one. Such a row reports FAIL "ambiguous background" instead of a
guess: the top colour must lead the second by at least 0.2 of the pixels, and the foreground must not fill the whole searched interior.
The measured margin on the 26 real rows is a lead of at least 0.375 (Tag/Chip, 0.615 against 0.24).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_ambiguity
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import crop_metrics as cm  # noqa: E402
from test_studio_ui_crop_metrics import BG, FILL, GREY, WHITE, grid, square  # noqa: E402


def two_tone(w, h, left_columns, a=FILL, b=BG, **shapes):
    """A w x h image whose first `left_columns` columns are colour a and the rest colour b, with extra rectangles on top."""
    px = [a if x < left_columns else b for _ in range(h) for x in range(w)]
    for x, y, sw, sh, c in shapes.values():
        for yy in range(y, y + sh):
            for xx in range(x, x + sw):
                px[yy * w + xx] = c
    return px


class LeadTest(unittest.TestCase):
    def test_the_lead_is_the_top_colour_share_minus_the_second_share(self):
        self.assertAlmostEqual(cm.surface_lead(two_tone(10, 10, 6), 10, 10), 0.2)
        self.assertAlmostEqual(cm.surface_lead(grid(10, 10), 10, 10), 1.0)

    def test_a_lead_of_exactly_0_2_is_clear_and_just_under_is_ambiguous(self):
        self.assertFalse(cm.ambiguous_surface(two_tone(10, 10, 6), 10, 10, inset=0))  # 60 against 40 pixels
        self.assertTrue(cm.ambiguous_surface(two_tone(100, 10, 59), 100, 10, inset=0))  # 590 against 410: lead 0.18

    def test_a_flat_image_and_a_normal_control_are_clear(self):
        self.assertFalse(cm.ambiguous_surface(grid(36, 36, FILL), 36, 36))
        self.assertFalse(cm.ambiguous_surface(square(10, WHITE), 36, 36))

    def test_the_foreground_filling_the_whole_interior_is_ambiguous_even_with_a_clear_lead(self):
        """A label that fills a tight crop: the surface it picks is the label, though the lead is 0.6."""
        px = grid(10, 8, BG, label=(1, 0, 8, 8, WHITE))
        self.assertGreaterEqual(cm.surface_lead(px, 10, 8), 0.2)
        self.assertTrue(cm.ambiguous_surface(px, 10, 8, inset=0))

    def test_a_line_as_wide_as_the_crop_is_not_the_whole_interior(self):
        """The separator: the box is as wide as the interior, but 1 px of 9 high, so the surface is the page."""
        self.assertFalse(cm.ambiguous_surface(grid(160, 17, BG, line=(0, 8, 160, 1, FILL)), 160, 17))


    def test_a_border_ring_around_a_faint_label_is_outside_the_searched_interior(self):
        """The 1 px border (54 from the page) is as strong as the faint label (50), so a search that reached the edge would take the whole crop as
        the foreground: the interior is inset, so the border is never part of it."""
        px = grid(40, 24, BG, label=(8, 8, 24, 8, (60, 60, 60)), top=(0, 0, 40, 1, (64, 64, 64)), bottom=(0, 23, 40, 1, (64, 64, 64)),
                  left=(0, 0, 1, 24, (64, 64, 64)), right=(39, 0, 1, 24, (64, 64, 64)))
        self.assertFalse(cm.ambiguous_surface(px, 40, 24))


class SyntheticRowsTest(unittest.TestCase):
    """The review's synthetic crops: each used to read as a wrong verdict."""

    def pair(self, render, prod, w, h, **entry):
        return cm.measure_pixels((render, w, h), (prod, w, h), entry)

    def test_a_glyph_covering_most_of_the_crop_in_both_images_fails_closed(self):
        """A filled mark of 31 px against production's 29 px in a 40 px crop: both images are glyph-majority, so the surface is the glyph and the
        boxes agree. It used to pass."""
        m = self.pair(grid(40, 40, BG, g=(4, 4, 31, 31, GREY)), grid(40, 40, BG, g=(5, 5, 29, 29, GREY)), 40, 40)
        self.assertTrue(m["ambiguous_render"] and m["ambiguous_prod"], m)
        self.assertTrue(m["ambiguous_fails"] and m["bad"], m)

    def test_a_label_filling_a_tight_crop_fails_closed(self):
        m = self.pair(grid(10, 8, BG, g=(1, 0, 8, 8, WHITE)), grid(10, 8, BG, g=(1, 1, 8, 6, WHITE)), 10, 8, fg_inset=0)
        self.assertTrue(m["ambiguous_fails"] and m["bad"], m)

    def test_a_two_tone_crop_split_evenly_fails_closed_and_says_so(self):
        """The same drawing shifted 1 px flipped the surface and read as 'surface 38 vs 10': it is now named ambiguous."""
        m = self.pair(two_tone(32, 16, 16, g=(20, 6, 4, 4, WHITE)), two_tone(32, 16, 15, g=(20, 6, 4, 4, WHITE)), 32, 16)
        self.assertTrue(m["ambiguous_fails"] and m["bad"], m)

    def test_a_clear_pair_is_not_ambiguous(self):
        m = self.pair(grid(60, 24, FILL, label=(14, 8, 32, 8, GREY)), grid(60, 24, FILL, label=(14, 8, 32, 8, GREY)), 60, 24)
        self.assertFalse(m["ambiguous_render"] or m["ambiguous_prod"] or m["ambiguous_fails"] or m["bad"], m)

    def test_one_ambiguous_image_is_enough(self):
        m = self.pair(grid(40, 40, BG, g=(4, 4, 31, 31, GREY)), grid(40, 40, BG, g=(14, 14, 12, 12, GREY)), 40, 40)
        self.assertTrue(m["ambiguous_render"] and not m["ambiguous_prod"] and m["ambiguous_fails"], m)

    def test_the_production_image_alone_being_ambiguous_fails_too(self):
        m = self.pair(grid(40, 40, BG, g=(14, 14, 12, 12, GREY)), grid(40, 40, BG, g=(4, 4, 31, 31, GREY)), 40, 40)
        self.assertTrue(m["ambiguous_prod"] and not m["ambiguous_render"] and m["ambiguous_fails"], m)

    def test_a_pending_entry_is_exempt_like_every_other_check(self):
        render, prod = grid(40, 40, BG, g=(4, 4, 31, 31, GREY)), grid(40, 40, BG, g=(5, 5, 29, 29, GREY))
        self.assertFalse(self.pair(render, prod, 40, 40, fg_pending="#1427")["bad"])
        enforced = cm.measure_pixels((render, 40, 40), (prod, 40, 40), {"fg_pending": "#1427"}, pending_open=False)
        self.assertTrue(enforced["ambiguous_fails"] and enforced["bad"], enforced)


class FailLineTest(unittest.TestCase):
    ROW = {"set": "Button/Icon", "variant": "Size=Sm, State=Default", "capture": "h-home/x", "fg_ok": True, "fg_render_px": (14, 14), "fg_production_px": (14, 14),
           "surface_ok": True, "surface_render": (10, 10, 10), "surface_production": (10, 10, 10), "peak_ok": True, "peak_colour_render": (163, 163, 163),
           "peak_colour_production": (163, 163, 163), "background_ok": True, "finding": None}

    def test_an_ambiguous_row_names_the_ambiguous_background(self):
        line = cm.fail_line({**self.ROW, "background_ok": False})
        self.assertIn("FAIL Button/Icon Size=Sm, State=Default h-home/x", line)
        self.assertIn("ambiguous background", line)

    def test_a_clear_row_does_not_mention_it(self):
        self.assertNotIn("ambiguous", cm.fail_line({**self.ROW, "fg_ok": False}))

    def test_the_box_surface_and_strongest_pixel_verdicts_are_listed(self):
        line = cm.fail_line({**self.ROW, "fg_ok": False, "surface_ok": False, "peak_ok": False, "finding": "a known gap"})
        self.assertIn("box FAIL", line)
        self.assertIn("surface FAIL", line)
        self.assertIn("strongest pixel FAIL", line)
        self.assertIn("finding: a known gap", line)


if __name__ == "__main__":
    unittest.main()
