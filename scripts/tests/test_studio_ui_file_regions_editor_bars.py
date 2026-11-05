"""The title bar, top tab bar and left rail regions (page `03 Studio`, row Regions) of components/regions_editor_bars.py, read from the IR the builders return against the real Semantic
plan: the record, its variant, the rect of the capture it is sized to, the place and name of the widgets layout.json names, and the neutral default content.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_editor_bars
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from regions_editor_fixtures import DENY, SIZE, SPECS, STYLES, build, by_name, default_build, rect  # noqa: E402,F401
from rest_ir import walk  # noqa: E402

AXES = {"Navigation/TitleBar": {}, "Navigation/TopTabBar": {"Bar": ["32px"]}, "Navigation/LeftRail": {"Rail": ["48px"], "Tool": ["File", "Agents", "Assets", "Variables"]}}


class RecordsTest(unittest.TestCase):
    def test_every_region_is_a_record_of_the_row_regions_with_the_registrys_axes(self):
        for name, axes in AXES.items():
            self.assertEqual((SPECS[name]["row"], SPECS[name]["axes"]), ("Regions", axes), name)

    def test_each_region_is_sized_to_the_rect_of_the_capture(self):
        for name in AXES:
            root = default_build(name)
            self.assertEqual((root["w"], root["h"]), tuple(float(v) for v in SIZE[name]), name)

    def test_the_regions_nest_only_what_they_declare_and_place_their_children_absolutely(self):
        for name in AXES:
            root = default_build(name)
            self.assertLessEqual({n["set"] for n in walk(root) if n["t"] == "X"}, set(SPECS[name].get("uses", [])), name)
            if not root.get("dir"):  # the tab strip is one auto layout row: its children flow
                self.assertTrue(all(k.get("abs") for k in root["kids"]), name)

    def test_every_style_exists_and_no_default_text_names_a_kit_client_or_library(self):
        for name in AXES:
            root = default_build(name)
            self.assertLessEqual({n["style"] for n in walk(root) if n["t"] == "T"}, STYLES, name)
            texts = [n["text"] for n in walk(root) if n["t"] == "T"] + [str(v) for n in walk(root) if n["t"] == "X" for v in n["props"].values()]
            self.assertEqual([t for t in texts if DENY.search(t)], [], name)


class RailAndTabsTest(unittest.TestCase):
    def test_the_rail_cells_sit_at_the_tops_of_cell_tops(self):
        rail = by_name(build("Navigation/LeftRail", Rail="48px"))
        tops = {"rail-File": 56, "rail-Agents": 112, "rail-Assets": 168, "rail-Tools": 224, "rail-Variables": 296, "rail-Plugins": 352}  # layout.json y minus the rail's top (32)
        for key, top in tops.items():
            self.assertEqual(rect(rail[key]), (0.0, float(top), 47.0, 56.0), key)

    def test_a_tile_is_the_32_px_tool_button_centred_on_its_cell_and_file_is_selected(self):
        rail = build("Navigation/LeftRail", Rail="48px")
        tiles = [n for n in walk(rail) if n["t"] == "X" and n["name"] == "Tile"]
        self.assertEqual([t["variant"] for t in tiles], ["State=Selected"] + ["State=Default"] * 5)
        self.assertEqual({rect(t) for t in tiles}, {(7.5, 12.0, 32.0, 32.0)})

    def test_the_menu_button_the_help_button_and_the_hairlines(self):
        rail = by_name(build("Navigation/LeftRail", Rail="48px"))
        self.assertEqual((rect(rail["rail-menu"]), rect(rail["rail-help"])), ((8.0, 8.0, 32.0, 32.0), (8.0, 828.0, 32.0, 32.0)))
        self.assertEqual((rect(rail["Menu separator"]), rect(rail["Group separator"]), rect(rail["Right border"])), ((8.0, 48.0, 31.0, 1), (8.0, 288.0, 31.0, 1), (48.0, 0, 1, 868.0)))

    def test_the_top_tab_bar_widgets(self):
        bar = by_name(build("Navigation/TopTabBar", Bar="32px"))
        """The strip is one auto layout row (tabs.rs:960-1121): 8 inset, Home 72, a gap of 3 after each child, the divider 3 into its 6 px slot, the project tab 240 and the `+` 2 into its 26 px
        slot, so the tab's x is 8 + 72 + 3 + 6 + 3 = 92 and the `+` 92 + 240 + 3 + 2 = 337, and 8 + 72 + 3 + 6 + 3 + 2 = 94 with the tab hidden (checked in `evidence screens`, h-home)."""
        root = build("Navigation/TopTabBar", Bar="32px")
        self.assertEqual((root["dir"], root["gap"][1:2], root["pad"][3][1:2]), ("H", (3.0,), (8.0,)))
        self.assertEqual([k["name"] for k in root["kids"]], ["tab-bar-home", "Home divider slot", "project-tab-0", "New tab slot", "Bottom line"])
        bar = by_name(root)
        self.assertEqual((bar["tab-bar-home"]["w"], bar["tab-bar-home"]["h"]), (72.0, 24.0))
        self.assertEqual((bar["project-tab-0"]["w"], bar["project-tab-0"]["h"], bar["project-tab-0"]["visible_prop"], bar["project-tab-0"].get("abs")), (240.0, 26.0, "Show project tab", None))
        self.assertEqual((rect(bar["Home divider"]), bar["Home divider slot"]["w"]), ((3, 0, 1, 16), 6.0))
        self.assertEqual((rect(bar["tab-bar-new"]), bar["New tab slot"]["w"]), ((2.0, 0.0, 24.0, 24.0), 26.0))
        self.assertEqual(rect(bar["Bottom line"]), (0, 31.0, 1440.0, 1))
        self.assertEqual(rect(bar["tab-close-1"]), (218.0, 4.0, 18.0, 18.0))  # 310 - 92: centred 13 px from the tab's right edge

    def test_the_title_bar_is_28_px_with_the_brand_at_76(self):
        bar = by_name(build("Navigation/TitleBar"))
        self.assertEqual(rect(bar["window-title-brand"])[:2], (76.0, 2.0))
        self.assertEqual(rect(bar["Bottom line"]), (0, 27.0, 1440.0, 1))



if __name__ == "__main__":
    unittest.main()
