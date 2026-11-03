"""The foreground bounding box, strongest pixel and max-difference figures of the component compare (no Pillow: pixels are RGB tuples in a flat list).

A whole-box mean difference scored a Close glyph drawn 14 px against production's 10 px at 1.41 of 255, so the compare also measures the
foreground (a glyph or a label) against the image's own surface and fails a size mismatch above 1 px. The surface colour, the pass/fail
decision and the exit code are tested in test_studio_ui_crop_verdict.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_metrics
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import crop_metrics as cm  # noqa: E402

BG, WHITE, GREY, FILL = (10, 10, 10), (255, 255, 255), (163, 163, 163), (38, 38, 38)
ORANGE, RED = (227, 99, 35), (220, 38, 38)


def grid(w, h, bg=BG, **shapes):
    """A w x h pixel list; shapes: name=(x, y, sw, sh, colour) rectangles painted in order."""
    px = [bg] * (w * h)
    for x, y, sw, sh, c in shapes.values():
        for yy in range(y, y + sh):
            for xx in range(x, x + sw):
                px[yy * w + xx] = c
    return px


def square(side, colour=WHITE, w=36, h=36, bg=BG, **more):
    x, y = (w - side) // 2, (h - side) // 2
    return grid(w, h, bg, glyph=(x, y, side, side, colour), **more)


class ForegroundBoxTest(unittest.TestCase):
    def test_the_box_is_the_glyph_extent(self):
        self.assertEqual(cm.bbox_size(cm.fg_bbox(square(10), 36, 36)), (10, 10))

    def test_a_14_px_glyph_against_a_10_px_crop_fails(self):
        render, prod = cm.fg_bbox(square(14), 36, 36), cm.fg_bbox(square(10), 36, 36)
        self.assertEqual(cm.mismatch(render, prod), {"d_width": 4, "d_height": 4, "ok": False})

    def test_a_one_pixel_difference_passes_and_two_fails(self):
        self.assertTrue(cm.mismatch(cm.fg_bbox(square(11), 36, 36), cm.fg_bbox(square(10), 36, 36))["ok"])
        self.assertFalse(cm.mismatch(cm.fg_bbox(square(12), 36, 36), cm.fg_bbox(square(10), 36, 36))["ok"])

    def test_the_colour_of_the_glyph_does_not_move_the_box(self):
        """A swapped glyph that renders white (engine bug) is the same extent as the grey one."""
        self.assertEqual(cm.fg_bbox(square(10, WHITE), 36, 36), cm.fg_bbox(square(10, GREY), 36, 36))

    def test_a_filled_control_with_rounded_corners_is_not_the_foreground(self):
        """Control fill 38, its corners show the page colour: only the glyph on the fill is measured."""
        px = grid(32, 32, FILL, glyph=(6, 6, 20, 20, WHITE))
        for x, y in ((0, 0), (1, 0), (0, 1), (31, 0), (0, 31), (31, 31), (30, 31), (31, 30)):
            px[y * 32 + x] = BG
        self.assertEqual(cm.bbox_size(cm.fg_bbox(px, 32, 32)), (20, 20))

    def test_a_border_ring_is_not_the_foreground(self):
        px = grid(40, 36, BG, top=(0, 0, 40, 1, (64, 64, 64)), bottom=(0, 35, 40, 1, (64, 64, 64)), left=(0, 0, 1, 36, (64, 64, 64)),
                  right=(39, 0, 1, 36, (64, 64, 64)), label=(12, 14, 16, 8, GREY))
        self.assertEqual(cm.bbox_size(cm.fg_bbox(px, 40, 36)), (16, 8))

    def test_a_dim_label_inside_a_brighter_border_is_still_the_foreground(self):
        ring = (90, 90, 90)
        px = grid(40, 36, BG, top=(0, 0, 40, 1, ring), bottom=(0, 35, 40, 1, ring), left=(0, 0, 1, 36, ring), right=(39, 0, 1, 36, ring),
                  label=(12, 14, 16, 8, (50, 50, 50)))
        self.assertEqual(cm.bbox_size(cm.fg_bbox(px, 40, 36)), (16, 8))

    def test_a_soft_edge_below_half_the_peak_is_not_the_foreground(self):
        px = grid(36, 36, BG, halo=(12, 12, 12, 12, (40, 40, 40)), glyph=(13, 13, 10, 10, WHITE))
        self.assertEqual(cm.bbox_size(cm.fg_bbox(px, 36, 36)), (10, 10))

    def test_a_halo_between_a_quarter_and_half_of_the_peak_is_not_the_foreground(self):
        """Peak distance 245: a halo at distance 90 is above a quarter (61) and below half (122), so the half-peak threshold excludes it."""
        px = grid(36, 36, BG, halo=(12, 12, 12, 12, (100, 100, 100)), glyph=(13, 13, 10, 10, WHITE))
        self.assertEqual(cm.bbox_size(cm.fg_bbox(px, 36, 36)), (10, 10))

    def test_a_pixel_just_above_half_of_the_peak_is_the_foreground(self):
        px = grid(36, 36, BG, soft=(12, 12, 12, 12, (140, 140, 140)), glyph=(13, 13, 10, 10, WHITE))
        self.assertEqual(cm.bbox_size(cm.fg_bbox(px, 36, 36)), (12, 12))

    def test_a_height_difference_alone_fails(self):
        wide = grid(36, 36, BG, glyph=(13, 11, 10, 14, WHITE))
        self.assertEqual(cm.mismatch(cm.fg_bbox(wide, 36, 36), cm.fg_bbox(square(10), 36, 36)), {"d_width": 0, "d_height": 4, "ok": False})

    def test_a_label_that_touches_the_image_edge_needs_inset_zero(self):
        """A hug-tight text node has no padding: the default inset clips 4 px off each side, so the entry sets `fg_inset` 0."""
        px = grid(37, 18, BG, label=(0, 5, 37, 9, GREY))
        self.assertEqual(cm.bbox_size(cm.fg_bbox(px, 37, 18)), (29, 9))
        self.assertEqual(cm.bbox_size(cm.fg_bbox(px, 37, 18, inset=0)), (37, 9))

    def test_an_image_with_no_foreground_has_no_box_and_two_empty_images_match(self):
        self.assertIsNone(cm.fg_bbox(grid(20, 20), 20, 20))
        self.assertTrue(cm.mismatch(None, None)["ok"])
        self.assertFalse(cm.mismatch(cm.fg_bbox(square(10), 36, 36), None)["ok"])


class PeakColourTest(unittest.TestCase):
    def test_the_strongest_pixel_of_the_foreground(self):
        self.assertEqual(cm.peak_colour(square(10, GREY), 36, 36), GREY)
        self.assertEqual(cm.peak_colour(grid(36, 36, FILL, glyph=(13, 13, 10, 10, WHITE)), 36, 36), WHITE)

    def test_no_foreground_has_no_colour(self):
        self.assertIsNone(cm.peak_colour(grid(20, 20), 20, 20))


class MaxDiffTest(unittest.TestCase):
    def test_the_largest_channel_difference_over_the_common_box(self):
        a, b = grid(4, 4), grid(4, 4)
        b[5] = (10, 10, 10 + 77)
        self.assertEqual(cm.max_diff(a, b, 4, 4), 77)
        self.assertEqual(cm.max_diff(a, a, 4, 4), 0)
        b[6] = (60, 60, 10)
        self.assertEqual(cm.max_diff(a, b, 4, 4), 77, "the largest single-channel difference, not a sum")


if __name__ == "__main__":
    unittest.main()
