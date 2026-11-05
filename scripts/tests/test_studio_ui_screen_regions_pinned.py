"""The screen gate averages each region's difference over the region's rect, so a cap is only as good as the rect: a rail rect that moved, or that is really the left panel's, still passes its cap
while the rail is never measured (measured: the rail 2 px off gives the same 2.385, the left panel's rect as "rail" gives 2.421 against a cap of 2.6). The four chrome regions are the capture's own
(`d-editor-default.layout.json` `regions`: tab_bar, rail, left_panel, right_panel), pinned here and, when the captures folder is on the machine, compared with the file itself. The ruler strips and
the toolbar are not layout regions: they come from the canvas rect, the strip width and the toolbar rect the module holds, and are pinned too.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_regions_pinned
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE.parent / "studio_ui_file" / "evidence"
sys.path.insert(0, str(EVIDENCE))
import screen_masks as sm  # noqa: E402
import capture_dirs  # noqa: E402

STATE = "d-editor-default"
LAYOUT_REGIONS = {"tab bar": ("tab_bar", (0, 0, 1440, 32)), "rail": ("rail", (0, 32, 49, 868)), "left panel": ("left_panel", (49, 32, 241, 868)),
                  "right panel": ("right_panel", (1199, 32, 241, 868))}
DERIVED = {"ruler top strip": (310, 32, 889, 20), "ruler left strip": (290, 52, 20, 848), "toolbar": (464, 840, 512, 48)}


def live_layout():
    """The capture's layout.json, or None when the captures folder is not on this machine (it is untracked)."""
    crops = json.loads((EVIDENCE / "crops.json").read_text())
    try:
        path = capture_dirs.captures_dir(crops["captures"], None) / f"{STATE}.layout.json"
    except Exception:  # no git, or a checkout with no main checkout beside it
        return None
    return json.loads(path.read_text()) if path.is_file() else None


class RegionRectTest(unittest.TestCase):
    def test_the_four_chrome_regions_are_the_layouts_rects(self):
        regions = sm.regions(STATE)
        for name, (_, rect) in LAYOUT_REGIONS.items():
            self.assertEqual(tuple(regions[name]), rect, name)

    def test_the_ruler_strips_and_the_toolbar_are_the_rects_the_canvas_gives_them(self):
        regions = sm.regions(STATE)
        for name, rect in DERIVED.items():
            self.assertEqual(tuple(regions[name]), rect, name)

    def test_no_two_regions_are_the_same_rect_and_the_set_is_the_seven_the_caps_name(self):
        regions = sm.regions(STATE)
        self.assertEqual(set(regions), set(LAYOUT_REGIONS) | set(DERIVED))
        self.assertEqual(len(set(map(tuple, regions.values()))), len(regions))

    def test_the_pinned_rects_are_the_ones_in_the_capture_when_it_is_here(self):
        layout = live_layout()
        if layout is None:
            self.skipTest("the 0.7.5 captures are not on this machine (they are untracked)")
        for name, (key, _) in LAYOUT_REGIONS.items():
            self.assertEqual(tuple(sm.regions(STATE)[name]), tuple(layout["regions"][key]), name)


if __name__ == "__main__":
    unittest.main()
