"""`00 Start Here` records how Popover/ColorPicker draws the inline `Color32` literals the app writes without a theme constant (the nearest theme tone by value, each mapping
named). The six lines are the ones the picker fix round gave, verbatim, and they sit in the component rules, before the `Gates` heading.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_start_here_picker
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import start_here  # noqa: E402
import studio_ui_gate_content as content  # noqa: E402

LINES = (
    "Inline colour literals (D5 deviation, Popover/ColorPicker): the app draws these from Color32 literals that no theme constant holds.",
    "The component draws the nearest theme tone by value, even a hover role: the largest per-channel difference against every opaque colour constant.",
    "Contrast ink 0xE4E4E7 -> TEXT_SECONDARY_HOVER (distance 2); badge text 0x4CC58B -> TEXT_SUCCESS (25).",
    "Badge surface 0x17302A -> BG_SUCCESS_PRIMARY (20, the nearest success surface; a neutral is 15 away but loses the green).",
    "Hue thumb 0xFF0000 -> BG_ERROR_SOLID (38); alpha thumb and thumb ring 0xFFFFFF -> ICON (0).",
    "Swatch edge white at alpha 0x1A -> BORDER_TRANSLUCENT (0). The thumb halo (black at alpha 0x4D) is omitted.",
)


class StartHerePickerTest(unittest.TestCase):
    def rows(self):
        plan = {"semantic": [], "primitives": []}
        return [(n["text_style"], n["text"]) for n in start_here.start_here(plan, 117, 18)["children"]]

    def test_the_six_lines_are_on_the_page_verbatim_in_the_body_style_in_order(self):
        rows = self.rows()
        first = rows.index(("Body / Regular", LINES[0]))
        self.assertEqual(rows[first:first + len(LINES)], [("Body / Regular", t) for t in LINES])

    def test_they_come_before_the_gates_heading(self):
        texts = [t for _, t in self.rows()]
        self.assertLess(texts.index(LINES[-1]), texts.index("Gates"))

    def test_no_line_is_longer_than_the_longest_line_the_page_already_had_and_none_names_a_denied_term(self):
        longest = max(len(t) for _, t in self.rows() if t not in LINES)
        self.assertLessEqual(max(len(t) for t in LINES), longest)
        self.assertEqual([d for d in content.DENY for t in LINES if d in t.lower()], [])


if __name__ == "__main__":
    unittest.main()
