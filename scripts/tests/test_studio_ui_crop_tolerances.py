"""The compare's colour tolerances and the #1427 exemption are pinned by numbers a mutated constant cannot satisfy, and the exemption lifts when
`swap_check.SWAP_CHECK_FATAL` flips (the decision is `crop_metrics.measure_pixels`'s default, read from that flag, so the call site has no
expression left to get wrong).

* Surface: 6 per channel. The two nearest theme surfaces are 13 apart (BG 10, BG_SECONDARY 23), so a tolerance above 12 would let one pass for
  the other. 6 and 7 pin the constant exactly.
* Strongest pixel: 8 per channel for the same drawing (every real row differs by 0 or 1), and 24 for a `colour_only` glyph, a custom painter's
  other drawing. A rail glyph drawn at 115 where production draws 163 (48 apart) is a wrong colour at either tolerance.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_tolerances
"""
import os
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import crop_metrics as cm  # noqa: E402
import swap_check  # noqa: E402
from test_studio_ui_crop_metrics import BG, WHITE, GREY, FILL, square  # noqa: E402


def with_surface(level, glyph=GREY):
    """A 36 px crop whose surface is the grey `level` and whose glyph is `glyph`."""
    return square(10, glyph, bg=(level, level, level))


def verdict(render, prod, **entry):
    return cm.measure_pixels((render, 36, 36), (prod, 36, 36), entry)


class SurfaceToleranceTest(unittest.TestCase):
    def test_the_nearest_two_theme_surfaces_are_not_the_same_surface(self):
        """BG (10) and BG_SECONDARY (23) are 13 apart: a tolerance of 27 or even 12 would call them one."""
        m = verdict(with_surface(10), with_surface(23))
        self.assertEqual(m["surface_diff"], 13)
        self.assertTrue(m["surface_fails"] and m["bad"], m)

    def test_six_levels_pass_and_seven_fail(self):
        self.assertFalse(verdict(with_surface(10), with_surface(16))["surface_fails"])
        self.assertTrue(verdict(with_surface(10), with_surface(17))["surface_fails"])

    def test_the_largest_channel_decides(self):
        render, prod = with_surface(10), square(10, GREY, bg=(10, 10, 17))
        self.assertTrue(verdict(render, prod)["surface_fails"])


class StrongestPixelToleranceTest(unittest.TestCase):
    def glyph(self, level):
        return with_surface(10, (level, level, level))

    def test_eight_levels_pass_and_nine_fail_for_the_same_drawing(self):
        self.assertFalse(verdict(self.glyph(171), self.glyph(163))["peak_fails"])
        self.assertTrue(verdict(self.glyph(172), self.glyph(163))["peak_fails"])

    def test_a_custom_painters_glyph_gets_24_levels_and_no_more(self):
        self.assertFalse(verdict(self.glyph(139), self.glyph(163), colour_only=True)["peak_fails"])
        self.assertTrue(verdict(self.glyph(138), self.glyph(163), colour_only=True)["peak_fails"])

    def test_a_rail_glyph_at_115_where_production_draws_163_fails_even_as_a_custom_painter(self):
        m = verdict(self.glyph(115), self.glyph(163), colour_only=True)
        self.assertEqual(m["peak_diff"], 48)
        self.assertTrue(m["peak_fails"] and m["bad"], m)

    def test_a_custom_painters_tolerance_does_not_leak_into_a_plain_entry(self):
        self.assertTrue(verdict(self.glyph(150), self.glyph(163))["peak_fails"])  # 13: fine for colour_only, not for a plain entry
        self.assertFalse(verdict(self.glyph(150), self.glyph(163), colour_only=True)["peak_fails"])


class PendingFlagTest(unittest.TestCase):
    """An `fg_pending` entry is exempt only while #1427 is open: swap_check.SWAP_CHECK_FATAL is False. Flipping it enforces the entry."""

    def wrong_pair(self):
        """A 14 px white glyph on the selected tile against production's 10 px grey one on the page: wrong box, surface and colour."""
        return (square(14, WHITE, bg=FILL), 36, 36), (square(10, GREY), 36, 36)

    def test_the_exemption_is_open_while_the_swap_check_is_not_fatal(self):
        render, prod = self.wrong_pair()
        with mock.patch.object(swap_check, "SWAP_CHECK_FATAL", False):
            m = cm.measure_pixels(render, prod, {"fg_pending": "#1427"})
        self.assertFalse(m["bad"], m)
        self.assertEqual(m["pending"], "#1427")

    def test_flipping_swap_check_fatal_enforces_the_entry(self):
        render, prod = self.wrong_pair()
        with mock.patch.object(swap_check, "SWAP_CHECK_FATAL", True):
            m = cm.measure_pixels(render, prod, {"fg_pending": "#1427"})
        self.assertTrue(m["bad"] and m["fg_fails"] and m["surface_fails"] and m["peak_fails"], m)
        self.assertIsNone(m["pending"])

    def test_an_explicit_argument_still_wins_over_the_flag(self):
        render, prod = self.wrong_pair()
        with mock.patch.object(swap_check, "SWAP_CHECK_FATAL", True):
            self.assertFalse(cm.measure_pixels(render, prod, {"fg_pending": "#1427"}, pending_open=True)["bad"])

    def test_the_issue_is_open_helper_reads_the_flag(self):
        with mock.patch.object(swap_check, "SWAP_CHECK_FATAL", False):
            self.assertTrue(cm.issue_open())
        with mock.patch.object(swap_check, "SWAP_CHECK_FATAL", True):
            self.assertFalse(cm.issue_open())


class StandaloneImportTest(unittest.TestCase):
    def test_crop_metrics_finds_swap_check_with_only_its_own_folder_on_the_path(self):
        """Every module of the suite runs alone (`Run:` in its docstring): importing crop_metrics must not depend on another test having put
        `scripts/studio_ui_file` on sys.path first."""
        evidence = HERE.parent / "studio_ui_file" / "evidence"
        done = subprocess.run([sys.executable, "-B", "-c", "import crop_metrics; print(crop_metrics.issue_open())"], cwd=evidence,
                              env={"PATH": os.environ["PATH"], "PYTHONPATH": str(evidence), "PYTHONDONTWRITEBYTECODE": "1"}, capture_output=True, text=True)
        self.assertEqual((done.returncode, done.stdout.strip()), (0, "True"), done.stderr[-400:])


if __name__ == "__main__":
    unittest.main()
