"""What the screen comparison leaves out (screen_masks) for `d-editor-default`: the canvas content and the strings a kit supplied, and nothing Studio draws itself. The rects are the capture's
(layout.json regions and named widgets, read once and written here), so the masks cannot grow over the chrome without a test failing.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_masks
"""
import re
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import screen_masks as sm  # noqa: E402

STATE = "d-editor-default"
# Studio's own chrome in that capture (layout.json): the toolbar, both ruler strips, the headers, the tabs, the type label column of the layer rows, the style sizes and weights.
CHROME = {"toolbar": (464, 840, 512, 48), "ruler top strip": (310, 32, 889, 20), "ruler left strip": (290, 52, 20, 848), "tab-bar-home": (8, 4, 72, 24),
          "pages-toggle": (49, 97, 240, 40), "layers header": (49, 305, 240, 40), "left-header-subtitle": (63, 71, 124, 18), "right-tab-Design": (1208, 80, 56, 24),
          "share-button": (1384, 40, 48, 24), "layer type label column": (183, 345, 50, 576), "style weight column": (1255, 313, 100, 32), "style size column": (1380, 313, 45, 32)}
DENY = re.compile(r"untitled|vsf|pcp|olivia|figma", re.I)


def overlaps(a, b):
    return a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and b[1] < a[1] + a[3]


class MaskTest(unittest.TestCase):
    def test_no_exclusion_covers_a_chrome_rect_studio_draws_itself(self):
        for name, rect in CHROME.items():
            hit = [e for e in sm.exclusions(STATE) if overlaps(e, rect)]
            self.assertEqual(hit, [], name)

    def test_the_canvas_content_is_excluded_around_the_toolbar_and_below_the_strips(self):
        ex = sm.exclusions(STATE)
        for point in ((400, 300), (1100, 300), (700, 500), (700, 894)):
            self.assertTrue(any(e[0] <= point[0] < e[0] + e[2] and e[1] <= point[1] < e[1] + e[3] for e in ex), point)
        self.assertFalse(any(e[0] <= 700 < e[0] + e[2] and e[1] <= 860 < e[1] + e[3] for e in ex), "the toolbar is chrome")

    def test_every_replaced_text_names_what_it_replaces_and_a_neutral_replacement(self):
        texts = sm.TEXTS[STATE]
        self.assertEqual(len(texts), 29)
        self.assertEqual({what for _, what, _ in texts}, {"file name", "project tab title", "page name", "layer name", "text style folder"})
        self.assertEqual([n for _, _, n in texts if DENY.search(n)], [])

    def test_the_regions_reported_tile_the_chrome_the_masks_leave_in(self):
        regions = sm.regions(STATE)
        self.assertEqual(set(regions), {"tab bar", "rail", "left panel", "right panel", "ruler top strip", "ruler left strip", "toolbar"})
        self.assertEqual(regions["right panel"], (1199, 32, 241, 868))


if __name__ == "__main__":
    unittest.main()
