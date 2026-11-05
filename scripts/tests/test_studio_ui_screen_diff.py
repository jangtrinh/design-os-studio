"""The pixel difference of a screen render against its capture over the pixels no mask excludes (screen_diff): the mean, the 99th percentile, the share above the notice level and the worst tile.
Flat lists of RGB tuples (no Pillow).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_diff
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import screen_diff as sd  # noqa: E402

W, H = 100, 80


def image(colour=(10, 10, 10), w=W, h=H):
    return [colour] * (w * h), w, h


def with_block(img, rect, colour):
    px, w, h = img
    px = list(px)
    x, y, bw, bh = rect
    for row in range(y, y + bh):
        for col in range(x, x + bw):
            px[row * w + col] = colour
    return px, w, h


class MaskTest(unittest.TestCase):
    def test_a_rect_is_excluded_and_clipped_to_the_image(self):
        m = sd.exclusion_mask(W, H, [(90, 70, 50, 50), (-5, -5, 10, 10)])
        self.assertEqual(sum(m), 10 * 10 + 5 * 5)
        self.assertEqual((m[0], m[5 * W + 5], m[75 * W + 95]), (1, 0, 1))

    def test_a_fractional_rect_is_excluded_whole_pixels_out(self):
        m = sd.exclusion_mask(W, H, [(10.5, 10, 5.25, 4)])
        self.assertEqual(sum(m), 6 * 4)


class StatsTest(unittest.TestCase):
    def test_equal_images_differ_by_nothing(self):
        d = sd.pixel_diffs(image(), image())
        s = sd.region_stats(d, W, None, sd.exclusion_mask(W, H, []))
        self.assertEqual((s["pixels"], s["mean"], s["p99"], s["share"]), (W * H, 0.0, 0, 0.0))

    def test_a_block_of_difference_is_counted_unless_masked(self):
        render = with_block(image(), (10, 10, 20, 10), (110, 10, 10))
        d, free, masked = sd.pixel_diffs(render, image()), sd.exclusion_mask(W, H, []), sd.exclusion_mask(W, H, [(10, 10, 20, 10)])
        a = sd.region_stats(d, W, None, free)
        self.assertEqual((a["mean"], a["p99"], a["share"]), (round(100 / 3 * 200 / (W * H), 3), 100, round(200 / (W * H), 4)))
        self.assertEqual(sd.region_stats(d, W, None, masked)["mean"], 0.0)
        self.assertEqual(sd.region_stats(d, W, None, masked)["pixels"], W * H - 200)

    def test_a_region_counts_only_its_own_pixels(self):
        render = with_block(image(), (10, 10, 20, 10), (110, 10, 10))
        d, free = sd.pixel_diffs(render, image()), sd.exclusion_mask(W, H, [])
        self.assertEqual(sd.region_stats(d, W, (10, 10, 20, 10), free)["share"], 1.0)
        self.assertEqual(sd.region_stats(d, W, (50, 50, 20, 10), free)["share"], 0.0)

    def test_a_small_difference_is_in_the_mean_and_not_in_the_share(self):
        d = sd.pixel_diffs(image((20, 20, 20)), image())
        s = sd.region_stats(d, W, None, sd.exclusion_mask(W, H, []))
        self.assertEqual((s["mean"], s["p99"], s["share"]), (10.0, 10, 0.0))

    def test_a_fully_masked_region_has_no_numbers(self):
        s = sd.region_stats(sd.pixel_diffs(image(), image()), W, (0, 0, 10, 10), sd.exclusion_mask(W, H, [(0, 0, 10, 10)]))
        self.assertEqual(s, {"pixels": 0, "mean": None, "p99": None, "share": None})

    def test_the_percentile_is_the_ceiling_of_99_percent(self):
        px = [(10, 10, 10)] * 100
        px[99] = (60, 10, 10)
        d = sd.pixel_diffs((px, 100, 1), image(w=100, h=1))
        self.assertEqual(sd.region_stats(d, 100, None, sd.exclusion_mask(100, 1, []))["p99"], 0)
        px[98] = (60, 10, 10)
        d = sd.pixel_diffs((px, 100, 1), image(w=100, h=1))
        self.assertEqual(sd.region_stats(d, 100, None, sd.exclusion_mask(100, 1, []))["p99"], 50)

    def test_images_of_different_sizes_fail_loudly(self):
        with self.assertRaises(ValueError):
            sd.pixel_diffs(image(w=10, h=10), image(w=10, h=11))


class TileTest(unittest.TestCase):
    def test_the_worst_tile_is_the_one_with_the_difference(self):
        d = sd.pixel_diffs(with_block(image(), (45, 5, 10, 10), (210, 10, 10)), image())
        t = sd.worst_tile(d, W, H, sd.exclusion_mask(W, H, []))
        self.assertEqual(t["rect"], [40, 0, 40, 40])

    def test_a_mostly_masked_tile_is_not_counted(self):
        d = sd.pixel_diffs(with_block(image(), (45, 5, 10, 10), (210, 10, 10)), image())
        t = sd.worst_tile(d, W, H, sd.exclusion_mask(W, H, [(40, 0, 40, 40)]))
        self.assertNotEqual(t["rect"], [40, 0, 40, 40])


if __name__ == "__main__":
    unittest.main()
