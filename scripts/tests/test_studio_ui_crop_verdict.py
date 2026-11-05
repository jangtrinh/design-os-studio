"""The compare's decision, tested without Pillow: every entry checks its foreground box, its surface colour (the colour most pixels have) and
its strongest-pixel colour, so a red Primary fill, a white shortcut label and a Default tool drawn on the Selected tile each fail. The
exemptions (`colour_only`, `fg_pending` while #1427 is open), the exit code and the foreground window are here too, so a widened
exemption, a colour check turned off and an exit code that is always 0 are each caught.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_verdict
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import crop_metrics as cm  # noqa: E402
from test_studio_ui_crop_metrics import BG, FILL, GREY, ORANGE, RED, WHITE, grid, square  # noqa: E402


class SurfaceTest(unittest.TestCase):
    def test_the_surface_is_the_most_common_colour(self):
        self.assertEqual(cm.surface_colour(grid(32, 32, FILL, glyph=(6, 6, 20, 20, WHITE)), 32, 32), FILL)
        self.assertEqual(cm.surface_colour(grid(36, 36, BG, glyph=(13, 13, 10, 10, GREY)), 36, 36), BG)

    def test_a_rounded_cap_crop_has_the_pill_as_surface(self):
        """The left 8 px of a pill: the top and bottom midpoints are anti-aliased edge pixels (a 2:2 tie with the left and right ones),
        so the surface is the colour most pixels have, not a sample of four edges (the sample read page and pill in turn)."""
        px = grid(8, 24, FILL)
        for y in range(24):
            for x in range(8):
                if (x == 0 and (y < 8 or y > 15)) or (x == 1 and (y < 4 or y > 19)) or (x < 4 and (y < 2 or y > 21)):
                    px[y * 8 + x] = BG
        px[2 * 8 + 4] = px[21 * 8 + 4] = (19, 19, 19)
        self.assertEqual(cm.surface_colour(px, 8, 24), FILL)

    def test_an_empty_image_has_no_surface(self):
        self.assertIsNone(cm.surface_colour([], 0, 0))


class ForegroundWindowTest(unittest.TestCase):
    def test_a_label_that_starts_at_the_edge_gets_its_padding_in_the_images_own_surface(self):
        """The shortcut crop: the render's label starts at x 1, so a 2 px pad clamped at the edge was 1 px short on one side only (a
        1 px misregistration read as a mean difference of 38). The pad is taken from the image padded with its own surface."""
        px = grid(10, 6, FILL, label=(0, 2, 10, 2, GREY))
        out, w, h = cm.fg_window(px, 10, 6, (0, 2, 10, 4), 2, FILL)
        self.assertEqual((w, h), (14, 6))
        self.assertEqual(out[0 * 14 + 0], FILL)
        self.assertEqual(out[2 * 14 + 1], FILL)
        self.assertEqual(out[2 * 14 + 2], GREY)
        self.assertEqual(out[2 * 14 + 11], GREY)
        self.assertEqual(out[2 * 14 + 12], FILL)

    def test_the_pad_is_the_fill_given_not_black(self):
        out, w, h = cm.fg_window(grid(4, 4, GREY), 4, 4, (0, 0, 2, 2), 2, FILL)
        self.assertEqual(out[0], FILL)

    def test_no_box_returns_the_image(self):
        px = grid(4, 3)
        self.assertEqual(cm.fg_window(px, 4, 3, None, 2, FILL), (px, 4, 3))


def button(surface, label, w=60, h=24):
    """A control: a flat surface with a 32 x 8 label."""
    return grid(w, h, surface, label=(14, 8, 32, 8, label))


def pair(render, prod, w=60, h=24, **entry):
    return cm.measure_pixels((render, w, h), (prod, w, h), entry)


class VerdictTest(unittest.TestCase):
    """Each of these is a drawing the whole-box mean let through, or a rule a widened exemption would hide."""

    def test_the_same_drawing_passes(self):
        m = pair(button(ORANGE, BG), button(ORANGE, BG))
        self.assertFalse(m["bad"], m)
        self.assertEqual((m["surface_diff"], m["peak_diff"], m["fg"]["ok"]), (0, 0, True))

    def test_a_red_primary_fill_fails_on_the_surface(self):
        m = pair(button(RED, BG), button(ORANGE, BG))
        self.assertTrue(m["surface_fails"] and m["bad"], m)
        self.assertEqual(m["surface_diff"], 61)

    def test_a_white_shortcut_label_fails_on_the_strongest_pixel(self):
        m = pair(button(BG, (250, 250, 250)), button(BG, GREY))
        self.assertTrue(m["peak_fails"] and m["bad"], m)
        self.assertFalse(m["surface_fails"])
        self.assertEqual(m["peak_diff"], 87)

    def test_a_default_tool_drawn_on_the_selected_tile_fails_on_the_surface(self):
        render, prod = grid(32, 32, FILL, glyph=(8, 8, 16, 16, GREY)), grid(32, 32, BG, glyph=(8, 8, 16, 16, GREY))
        m = cm.measure_pixels((render, 32, 32), (prod, 32, 32), {})
        self.assertTrue(m["surface_fails"] and m["bad"], m)
        self.assertEqual((m["surface_diff"], m["peak_diff"]), (28, 0))

    def test_a_partial_recolour_fails_unless_the_glyph_is_a_custom_painters(self):
        near = pair(button(BG, (200, 200, 200)), button(BG, (212, 212, 212)))
        self.assertTrue(near["peak_fails"] and near["bad"], near)
        custom = pair(button(BG, (200, 200, 200)), button(BG, (212, 212, 212)), colour_only=True)
        self.assertFalse(custom["bad"], custom)

    def test_a_box_mismatch_fails_a_plain_entry(self):
        m = pair(square(14), square(10), w=36, h=36)
        self.assertTrue(m["fg_fails"] and m["bad"], m)
        self.assertEqual((m["fg"]["d_width"], m["fg"]["d_height"]), (4, 4))

    def test_colour_only_exempts_the_box_but_not_the_colour(self):
        self.assertFalse(pair(square(14), square(10), w=36, h=36, colour_only=True)["bad"])
        m = pair(square(14, WHITE), square(10, GREY), w=36, h=36, colour_only=True)
        self.assertTrue(m["bad"] and m["peak_fails"] and not m["fg_fails"], m)
        m = pair(square(10, GREY, bg=FILL), square(10, GREY), w=36, h=36, colour_only=True)
        self.assertTrue(m["bad"] and m["surface_fails"], m)

    def test_a_pending_entry_is_exempt_from_the_box_and_the_colour_while_the_issue_is_open(self):
        m = pair(square(14, WHITE, bg=FILL), square(10, GREY), w=36, h=36, fg_pending="#1427")
        self.assertFalse(m["bad"], m)
        self.assertEqual(m["pending"], "#1427")
        self.assertFalse(m["fg_fails"] or m["surface_fails"] or m["peak_fails"])

    def test_a_pending_entry_is_enforced_once_the_issue_is_closed(self):
        m = cm.measure_pixels((square(14, WHITE, bg=FILL), 36, 36), (square(10, GREY), 36, 36), {"fg_pending": "#1427"}, pending_open=False)
        self.assertTrue(m["bad"] and m["fg_fails"] and m["surface_fails"] and m["peak_fails"], m)
        self.assertIsNone(m["pending"])

    def test_a_foreground_on_one_side_only_fails(self):
        m = pair(grid(36, 36, BG), square(10, GREY), w=36, h=36)
        self.assertTrue(m["bad"] and m["peak_fails"], m)

    def test_two_flat_images_of_the_same_colour_pass(self):
        self.assertFalse(pair(grid(36, 36, FILL), grid(36, 36, FILL), w=36, h=36)["bad"])


class OutcomeTest(unittest.TestCase):
    GOOD = {"set": "Button/Primary", "variant": "v", "capture": "h-home/x", "bad": False, "pending": None}
    BAD = {"set": "Button/Icon", "variant": "v", "capture": "h-home/y", "bad": True, "pending": None}
    PENDING = {"set": "Button/Tool", "variant": "v", "capture": "d-editor-default/z", "bad": False, "pending": "#1427"}

    def test_a_clean_run_exits_0(self):
        self.assertEqual(cm.exit_code([self.GOOD, self.PENDING]), 0)

    def test_one_bad_row_exits_1_and_is_listed(self):
        self.assertEqual(cm.exit_code([self.GOOD, self.BAD]), 1)
        self.assertEqual(cm.bad_rows([self.GOOD, self.BAD, self.PENDING]), [self.BAD])

    def test_nothing_compared_is_not_a_pass(self):
        self.assertEqual(cm.exit_code([]), 2)

    def test_the_pending_rows_are_named_in_the_output(self):
        note = cm.pending_note([self.GOOD, self.PENDING, self.PENDING])
        self.assertIn("2 of 3", note)
        self.assertIn("#1427", note)
        self.assertIn("box and colour", note)
        self.assertIn("Button/Tool v d-editor-default/z", note)
        self.assertIsNone(cm.pending_note([self.GOOD]))


if __name__ == "__main__":
    unittest.main()
