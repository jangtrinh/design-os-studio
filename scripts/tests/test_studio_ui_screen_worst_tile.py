"""The worst 40 px tile of a screen (screen_diff.worst_tile): a tile counts when at least half of it is unmasked, and its mean is taken over its unmasked pixels only. The cap on the worst
tile (34) is only as strict as that rule: a rule that asked for three quarters would skip a tile that is 60 % unmasked and hold the worst difference, and the gate would report a better
tile. Flat lists of RGB tuples (no Pillow).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_worst_tile
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import screen_diff as sd  # noqa: E402

BASE, RED_90, RED_30 = (10, 10, 10), (100, 10, 10), (40, 10, 10)  # a channel difference of 90 is a mean of 30 over the three channels, 30 is 10


def pair(w, h, blocks):
    """(render, capture) of w x h: the capture is flat, the render differs from it in each (rect, colour) of `blocks`."""
    px = [BASE] * (w * h)
    for (x, y, bw, bh), colour in blocks:
        for row in range(y, y + bh):
            px[row * w + x:row * w + x + bw] = [colour] * bw
    return (px, w, h), ([BASE] * (w * h), w, h)


def worst(w, h, blocks, masked=()):
    render, capture = pair(w, h, blocks)
    return sd.worst_tile(sd.pixel_diffs(render, capture), w, h, sd.exclusion_mask(w, h, list(masked)))


class WorstTileTest(unittest.TestCase):
    def test_a_tile_that_is_60_percent_unmasked_is_measured_over_its_unmasked_pixels(self):
        """Left tile: the top 16 rows (40 %) masked, the 24 rows below differ by a mean of 30. Right tile: unmasked, a mean of 10."""
        got = worst(80, 40, [((0, 16, 40, 24), RED_90), ((40, 0, 40, 40), RED_30)], masked=[(0, 0, 40, 16)])
        self.assertEqual(got, {"rect": [0, 0, 40, 40], "mean": 30.0})

    def test_a_tile_that_is_exactly_half_unmasked_still_counts(self):
        got = worst(80, 40, [((0, 20, 40, 20), RED_90), ((40, 0, 40, 40), RED_30)], masked=[(0, 0, 40, 20)])
        self.assertEqual(got, {"rect": [0, 0, 40, 40], "mean": 30.0})

    def test_a_tile_that_is_less_than_half_unmasked_is_skipped_however_bad_it_is(self):
        """Left tile: 24 of 40 rows masked (40 % unmasked) and the 16 left differ by a mean of 30; the right tile's mean of 10 is the worst that counts."""
        got = worst(80, 40, [((0, 24, 40, 16), RED_90), ((40, 0, 40, 40), RED_30)], masked=[(0, 0, 40, 24)])
        self.assertEqual(got, {"rect": [40, 0, 40, 40], "mean": 10.0})

    def test_an_edge_tile_is_the_size_that_is_left_and_is_judged_against_its_own_area(self):
        """A 100 x 40 image has a right tile of 20 x 40 = 800 px: half of it is 400 px, so 11 unmasked rows of 20 (220 px) skip it and 20 rows (400 px) count it."""
        blocks = [((80, 0, 20, 40), RED_90)]
        self.assertEqual(worst(100, 40, blocks, masked=[(80, 0, 20, 29)])["mean"], 0.0)
        self.assertEqual(worst(100, 40, blocks, masked=[(80, 0, 20, 20)]), {"rect": [80, 0, 20, 40], "mean": 30.0})

    def test_every_tile_masked_leaves_no_worst_tile(self):
        self.assertEqual(worst(80, 40, [((0, 0, 80, 40), RED_90)], masked=[(0, 0, 80, 40)]), {"rect": None, "mean": -1.0})

    def test_the_first_of_two_equal_tiles_is_the_one_reported(self):
        got = worst(80, 40, [((0, 0, 40, 40), RED_90), ((40, 0, 40, 40), RED_90)])
        self.assertEqual(got["rect"], [0, 0, 40, 40])


if __name__ == "__main__":
    unittest.main()
