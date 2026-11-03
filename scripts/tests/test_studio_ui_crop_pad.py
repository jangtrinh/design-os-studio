"""A bare 16 px swatch has no surface of its own: the crop of a production swatch carries the field around it, so the render is padded with the same
surface to the crop's size (`render_pad` [left, top, right, bottom] in crops.json) before the two are compared. The padding is pure pixel work here
so the suite reaches it without Pillow. Without it a flat swatch is "one mark covering the whole interior" and fails closed.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_pad
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import crop_metrics as cm  # noqa: E402
from test_studio_ui_crop_metrics import FILL, GREY, grid  # noqa: E402


class PadImageTest(unittest.TestCase):
    def test_the_image_grows_by_the_given_margins_in_the_surface_colour(self):
        px, w, h = cm.pad_image([GREY] * 4, 2, 2, (1, 2, 3, 4), FILL)
        self.assertEqual((w, h, len(px)), (6, 8, 48))
        self.assertEqual(px[2 * w + 1], GREY)  # the original (0, 0) now at (1, 2)
        self.assertEqual({px[0], px[w * h - 1]}, {FILL})

    def test_no_margin_returns_the_same_pixels(self):
        px = grid(3, 3, GREY)
        self.assertEqual(cm.pad_image(px, 3, 3, (0, 0, 0, 0), FILL), (px, 3, 3))

    def test_a_padded_swatch_has_a_foreground_box_and_a_clear_surface(self):
        edge = (190, 190, 190)  # the translucent border: a 1 px ring round a flat colour, so the interior is flat but the image is not
        swatch = grid(16, 16, GREY, top=(0, 0, 16, 1, edge), bottom=(0, 15, 16, 1, edge), left=(0, 0, 1, 16, edge), right=(15, 0, 1, 16, edge))
        self.assertTrue(cm.ambiguous_surface(swatch, 16, 16))
        px, w, h = cm.pad_image(swatch, 16, 16, (4, 8, 8, 8), FILL)
        self.assertEqual((w, h), (28, 32))
        self.assertEqual(cm.bbox_size(cm.fg_bbox(px, w, h)), (16, 16))
        self.assertFalse(cm.ambiguous_surface(px, w, h))

    def test_a_negative_margin_is_refused(self):
        with self.assertRaises(ValueError):
            cm.pad_image([GREY], 1, 1, (-1, 0, 0, 0), FILL)


if __name__ == "__main__":
    unittest.main()
