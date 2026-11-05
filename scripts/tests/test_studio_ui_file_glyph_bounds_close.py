"""`Close` returns the pen to the start of the subpath, so a curve that follows it starts there (`glyph_bounds.path_bounds`). The W1a fix-3
review's mutant that left the pen at the last point survived the suite because no test drew a curve right after a `Close`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_glyph_bounds_close
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import glyph_bounds as gb  # noqa: E402


def pt(x, y):
    return {"x": x, "y": y}


TRIANGLE = [{"MoveTo": pt(0, 0)}, {"LineTo": pt(10, 0)}, {"LineTo": pt(10, 10)}, "Close"]


def bounds(*extra):
    return gb.path_bounds([{"cmds": TRIANGLE + list(extra), "fill_rule": "NonZero"}])


class CloseReturnsToStartTest(unittest.TestCase):
    def test_a_quadratic_after_close_starts_at_the_subpath_start(self):
        """From (0, 0) towards the handle (-20, 0) and the end (0, 20), x(t) = -40 t (1 - t): its leftmost point is -10 at t = 1/2, a sampled
        point. From the last point (10, 10) the same command only reaches about -7.5."""
        x0, y0, x1, y1 = bounds({"QuadTo": [pt(-20, 0), pt(0, 20)]})
        self.assertAlmostEqual(x0, -10.0, places=6)
        self.assertEqual((y0, x1, y1), (0.0, 10.0, 20.0))

    def test_a_cubic_after_close_starts_at_the_subpath_start(self):
        """From (0, 0) with handles (-8, 0), (-8, 8) and the end (0, 8): x(1/2) = -6 exactly. From (10, 10) it would stay right of x -3."""
        x0, y0, x1, y1 = bounds({"CubicTo": [pt(-8, 0), pt(-8, 8), pt(0, 8)]})
        self.assertAlmostEqual(x0, -6.0, places=6)
        self.assertEqual((y0, x1, y1), (0.0, 10.0, 10.0))

    def test_close_with_nothing_after_it_adds_no_point(self):
        self.assertEqual(bounds(), (0.0, 0.0, 10.0, 10.0))

    def test_a_close_with_no_subpath_start_is_ignored(self):
        """A `Close` before any `MoveTo` has no start to return to and no point to add."""
        self.assertIsNone(gb.path_bounds([{"cmds": ["Close"], "fill_rule": "NonZero"}]))


if __name__ == "__main__":
    unittest.main()
