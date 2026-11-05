"""`00 Start Here` states the rules of the regions and screens (W3): a region is sized to the capture's rect and names its widgets by the layout.json key; a screen is instances of
  regions plus the
page's colour; a screen shows no kit content; two engine and component limits that make a region differ from production. The text comes from `start_here.start_here`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_start_here_regions
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import start_here  # noqa: E402


def lines():
    return [n["text"] for n in start_here.start_here({"semantic": [], "primitives": []}, 117, 18)["children"]]


class StartHereRegionsTest(unittest.TestCase):
    def test_the_region_rules_are_there(self):
        text = " ".join(lines()).lower()
        for part in ("regions", "sized to its rect in the capture's layout.json", "carries the layout.json key as its node name", "instances only"):
            self.assertIn(part, text, part)

    def test_the_screen_rules_are_there(self):
        text = " ".join(lines()).lower()
        for part in ("04 screens", "named after its registry screen record", "page's own colour", "no kit, client or library name"):
            self.assertIn(part, text, part)

    def test_the_two_limits_are_named_with_their_issues(self):
        text = " ".join(lines())
        self.assertIn("#1427", text)
        self.assertIn("#1471", text)

    def test_the_shared_headers_say_how_they_choose_their_glyphs_and_the_old_gap_is_gone(self):
        text = " ".join(lines())
        for part in ("Show Collapse action", "Show Add action", "Show Opacity action"):
            self.assertIn(part, text, part)
        self.assertNotIn("shows the Pages pair where production draws ChevronDown and Search", text)

    def test_the_screen_gate_and_the_alpha_rule_are_stated(self):
        text = " ".join(lines())
        for part in ("a cap on the mean difference of each region", "worst 40 px tile", "99th percentile is reported", "gamma_multiply(0.22)"):
            self.assertIn(part, text, part)

    def test_the_old_sentence_that_04_screens_is_empty_is_gone(self):
        self.assertNotIn("04 Screens is empty", " ".join(lines()))

    def test_the_gate_lines_stay_last(self):
        self.assertEqual(lines()[-2:], ["Gates", "python3 scripts/check_studio_ui_file.py design/studio-ui.designos"])


if __name__ == "__main__":
    unittest.main()
