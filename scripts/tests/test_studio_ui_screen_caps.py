"""The screen gate: a mean-difference cap per region of every screen, and a cap on the worst tile (screen_caps).

The caps are what the first screen measured plus about 8 % (the W3a numbers: screen 4.24, tab bar 0.29, rail 2.39, left panel 2.50, right panel 6.33, ruler strips 6.40 and 6.57,
toolbar 3.50, worst 40 px tile 31.2), per region because a region moved 2 px barely moves the whole screen's mean (the screen mean alone catches a panel moved 2 px at 4.96 and misses
the rail at 4.34). The 99th percentile is never capped: it is text-edge dominated (86 to 181 on this screen, the same before and after a 2 px move) and is reported only. A screen, a
region or a cap that does not pair up fails, and so does a region with no counted pixel: an empty selection never passes.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_caps
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import screen_caps as caps  # noqa: E402
import screen_diff as sd  # noqa: E402

MEASURED = {"screen": 4.240, "tab bar": 0.293, "rail": 2.385, "left panel": 2.498, "right panel": 6.333, "ruler top strip": 6.399, "ruler left strip": 6.574, "toolbar": 3.498}


def scores(means=None, tile=31.2, state="d-editor-default", p99=181, pixels=1000):
    means = dict(MEASURED, **(means or {}))
    stat = lambda m: {"pixels": pixels, "mean": m, "p99": p99, "share": 0.04}
    return {"screen": "Screen/EditorDesign", "state": state, "pixels": stat(means["screen"]), "regions": {n: stat(m) for n, m in means.items() if n != "screen"},
            "worst_tile": {"rect": [1240, 520, 40, 40], "mean": tile}, "geometry": {"mismatched": []}}


class CapsTest(unittest.TestCase):
    def test_the_first_screens_measured_numbers_pass(self):
        self.assertEqual(caps.judge(scores()), [])

    def test_the_caps_are_the_measured_means_plus_about_eight_percent(self):
        table = caps.CAPS["d-editor-default"]
        self.assertEqual(table, {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8})
        self.assertEqual(caps.WORST_TILE, 34.0)
        for name, cap in table.items():
            self.assertLess(MEASURED[name], cap, name)
            self.assertLess(cap / MEASURED[name], 1.5, f"{name}: a cap this far above the measure would not see a 2 px move")

    def test_each_region_just_over_its_cap_fails_and_is_named_with_both_numbers(self):
        for name, cap in caps.CAPS["d-editor-default"].items():
            over = caps.judge(scores({name: cap + 0.01}))
            self.assertEqual(len(over), 1, name)
            self.assertIn(name, over[0])
            self.assertIn(str(cap), over[0])
            self.assertEqual(caps.judge(scores({name: cap})), [], f"{name}: the cap itself passes")

    def test_a_worst_tile_over_34_fails(self):
        self.assertEqual(caps.judge(scores(tile=34.0)), [])
        out = caps.judge(scores(tile=34.1))
        self.assertEqual(len(out), 1)
        self.assertIn("worst", out[0])

    def test_the_99th_percentile_is_reported_never_capped(self):
        self.assertEqual(caps.judge(scores(p99=255)), [])

    def test_a_screen_with_no_caps_fails(self):
        state = "g-spec-reader"
        self.assertNotIn(state, caps.CAPS, f"{state!r} now has caps: pick an uncapped state")
        out = caps.judge(scores(state=state))
        self.assertEqual(len(out), 1)
        self.assertIn(state, out[0])

    def test_a_region_with_no_cap_and_a_cap_with_no_region_both_fail(self):
        s = scores()
        s["regions"]["new panel"] = dict(s["regions"]["rail"])
        self.assertTrue(any("new panel" in line and "no cap" in line for line in caps.judge(s)))
        s = scores()
        del s["regions"]["toolbar"]
        self.assertTrue(any("toolbar" in line and "not in the scores" in line for line in caps.judge(s)))

    def test_a_region_with_no_counted_pixel_fails_instead_of_passing_empty(self):
        s = scores()
        s["regions"]["rail"] = {"pixels": 0, "mean": None, "p99": None, "share": None}
        self.assertTrue(any("rail" in line and "no counted pixel" in line for line in caps.judge(s)))
        s = scores()
        s["pixels"] = {"pixels": 0, "mean": None, "p99": None, "share": None}
        self.assertTrue(any("screen" in line and "no counted pixel" in line for line in caps.judge(s)))

    def test_a_missing_worst_tile_fails(self):
        s = scores()
        s["worst_tile"] = {"rect": None, "mean": -1.0}
        self.assertTrue(any("worst" in line for line in caps.judge(s)))


class ExitCodeTest(unittest.TestCase):
    def test_no_screen_is_exit_2_a_pass_is_0(self):
        self.assertEqual((caps.exit_code([]), caps.exit_code([scores()])), (2, 0))

    def test_a_failed_cap_is_exit_1_as_is_a_widget_rect_off_by_more_than_a_pixel(self):
        self.assertEqual(caps.exit_code([scores({"rail": 2.7})]), 1)
        off = scores()
        off["geometry"]["mismatched"] = [{"key": "rail-File"}]
        self.assertEqual(caps.exit_code([scores(), off]), 1)


class ShiftTest(unittest.TestCase):
    """A region moved 2 px is over its cap, the unmoved one is not: the property the per-region caps exist for, on synthetic text-like stripes through the real diff."""
    W, H = 240, 120

    def stripes(self, offset=0):
        px = [((255, 255, 255) if (x + offset) % 5 < 2 else (10, 10, 10)) for y in range(self.H) for x in range(self.W)]
        return px, self.W, self.H

    def mean(self, shifted):
        render, capture = self.stripes(2 if shifted else 0), self.stripes()
        return sd.region_stats(sd.pixel_diffs(render, capture), self.W, (0, 0, self.W, self.H), sd.exclusion_mask(self.W, self.H, []))["mean"]

    def test_the_unmoved_region_passes_and_the_two_pixel_move_is_over_every_cap(self):
        self.assertEqual(self.mean(False), 0.0)
        moved = self.mean(True)
        for name, cap in caps.CAPS["d-editor-default"].items():
            self.assertGreater(moved, cap, name)
            self.assertEqual(len(caps.judge(scores({name: moved}))), 1, name)


if __name__ == "__main__":
    unittest.main()
