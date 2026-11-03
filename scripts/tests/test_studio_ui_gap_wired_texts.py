"""The rects the wired screens leave out of the comparison (`screens/gap_wired_texts.json`) are the ones their data modules imply, every state of them is declared by the gate module that owns
the state, and no rect leaves the window.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_wired_texts
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import studio_ui_gate_screen_data as gate_data  # noqa: E402
from screens import gap_left_dev_data as left_dev  # noqa: E402
from screens import gap_wired_texts as wired  # noqa: E402

DATA = json.loads(wired.PATH.read_text(encoding="utf-8"))
TEXTS = gate_data.load()["TEXTS"]


class WiredTextsTest(unittest.TestCase):
    def test_the_file_is_what_the_data_modules_imply(self):
        self.assertEqual(wired.PATH.read_text(encoding="utf-8"), wired.render(wired.build()), "regenerate it: python3 scripts/studio_ui_file/screens/gap_wired_texts.py")

    def test_every_rect_is_declared_by_the_gate_module_that_owns_its_state(self):
        for state, rects in DATA.items():
            declared = [(list(r), what, neutral) for r, what, neutral in TEXTS[state]]
            for x, y, w, h, what, neutral in rects:
                self.assertIn(([x, y, w, h], what, neutral), declared, f"{state}: {what}")

    def test_no_rect_leaves_the_window_and_every_text_says_what_it_replaces(self):
        for state, rects in DATA.items():
            for x, y, w, h, what, neutral in rects:
                self.assertTrue(0 <= x and 0 <= y and w > 0 and h > 0 and x + w <= 1440 and y + h <= 900, (state, what, (x, y, w, h)))
                self.assertTrue(what and isinstance(neutral, str), state)  # a rect the screen draws nothing in (the style description) has the empty string

    def test_a_state_with_a_layer_tree_masks_the_name_of_every_row_it_draws_inside_the_window(self):
        for state, rects in left_dev.LEFT_TEXTS.items():
            drawn = [r for r in DATA[state] if r[4] == "layer name"]
            self.assertEqual(len(drawn), len([r for r in rects if r[0][1] < 900]), state)

    def test_the_kit_names_of_the_selection_inspector_are_masked_on_every_state_that_shows_one(self):
        from screens import gap_right_panel_data as right
        for state, entry in right.RIGHT_PANEL.items():
            has_name = any(r[4] == "selected layer name" for r in DATA.get(state, []))
            self.assertEqual(has_name, entry["variant"]["State"] in wired.NAMED_STATES, state)


if __name__ == "__main__":
    unittest.main()
