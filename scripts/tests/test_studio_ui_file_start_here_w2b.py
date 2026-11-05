"""`00 Start Here` carries the two rules the W2b composites need (controller notes 2026-10-31 19:00 and 19:40): sticky Disabled, and the disabled segmented
strip as a recorded deviation. The text comes from `start_here.start_here`, the generator of the page.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_start_here_w2b
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import start_here  # noqa: E402


class StartHereTest(unittest.TestCase):
    """`00 Start Here` carries the two rules the W2b composites need (controller notes 2026-10-31 19:00 and 19:40)."""

    def page_text(self):
        """The page's lines as the generator writes them, case kept: a sentence the review gave is compared to this exactly."""
        plan = {"semantic": [], "primitives": []}
        return " ".join(n["text"] for n in start_here.start_here(plan, 117, 18)["children"])

    def words(self):
        return self.page_text().lower()

    def test_the_sticky_disabled_sentence_is_the_one_the_fix_2_review_gave(self):
        sentence = ("Disabled is sticky: every paint inside a Disabled variant, or inside a nested instance of a Disabled variant at any depth, carries 0.5. "
                    "A non-Disabled variant may nest a Disabled control, and a Disabled variant nests only the Disabled variant of each control it holds "
                    "(an Icon glyph is the one exception).")
        self.assertIn(sentence, self.page_text())

    def test_the_disabled_segmented_strip_is_recorded_as_a_deviation(self):
        words = self.words()
        for part in ("tabs/segmented state=disabled", "the app never draws a disabled segmented", "both cells take segmented_look(disabled)", "shell is at 0.5"):
            self.assertIn(part, words, part)

    def test_the_older_disabled_rule_is_still_there(self):
        for part in ("half alpha per paint", "opacity 0.5"):
            self.assertIn(part, self.words(), part)


if __name__ == "__main__":
    unittest.main()
