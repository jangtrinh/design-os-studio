"""The rulers and bottom canvas toolbar regions (page `03 Studio`, row Regions) of components/regions_editor_canvas.py, read from the IR the builders return against the real Semantic plan:
the record, the rect of the capture it is sized to, the place of every widget layout.json names, the ticks as vector paths, and the neutral default content.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_editor_canvas
"""
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from regions_editor_fixtures import DENY, SIZE, SPECS, STYLES, build, by_name, default_build, rect  # noqa: E402,F401
from rest_ir import walk  # noqa: E402

AXES = {"Navigation/Rulers": {}, "Navigation/BottomCanvasToolbar": {"State": ["Move", "Comment", "AI", "Dev", "DevAnnotate"]}}


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
            self.assertTrue(all(k.get("abs") for k in root["kids"]), name)

    def test_every_style_exists_and_no_default_text_names_a_kit_client_or_library(self):
        for name in AXES:
            root = default_build(name)
            self.assertLessEqual({n["style"] for n in walk(root) if n["t"] == "T"}, STYLES, name)
            texts = [n["text"] for n in walk(root) if n["t"] == "T"] + [str(v) for n in walk(root) if n["t"] == "X" for v in n["props"].values()]
            self.assertEqual([t for t in texts if DENY.search(t)], [], name)


class ToolbarAndRulersTest(unittest.TestCase):
    def test_the_tool_groups_chevrons_actions_divider_and_switcher_sit_where_toolbar_rs_puts_them(self):
        bar = by_name(build("Navigation/BottomCanvasToolbar"))
        self.assertEqual([rect(bar[f"tool-{i}"])[0] for i in range(6)], [8.0, 64.0, 120.0, 176.0, 232.0, 288.0])
        self.assertEqual([rect(bar[f"tool-chevron-{i}"])[0] for i in range(5)], [40.0, 96.0, 152.0, 208.0, 264.0])
        self.assertNotIn("tool-chevron-5", bar, "the comment tool has no dropdown")
        self.assertEqual(rect(bar["toolbar-actions-btn"]), (328.0, 8.0, 32.0, 32.0))
        self.assertEqual(rect(bar["Divider"]), (373.0, 1, 1, 46))
        self.assertEqual(rect(bar["Mode switcher"]), (382.0, 8.0, 122.0, 32.0))

    def test_the_mode_cells_split_the_pill_in_three_and_design_is_selected(self):
        bar = by_name(build("Navigation/BottomCanvasToolbar"))
        cells = [bar[k] for k in ("toolbar-mode-draw", "toolbar-mode-design", "toolbar-mode-dev")]
        self.assertEqual([round(c["abs"][2], 2) for c in cells], [39.33] * 3)
        self.assertEqual([bool(c.get("fill")) for c in cells], [False, True, False])

    def test_the_highlight_stroke_is_the_bars_second_row_and_column_and_its_last(self):
        """d-editor-default: the 1 px INNER_HIGHLIGHT line is on the bar's rows 1 and 47 and columns 1 and 511 (egui paints it on `bar.shrink(0.5)` and snaps the top and left
        half pixel inward), so the stroke sits on a (1, 1, 511, 47) frame and the bar itself has none: its edge row and column are the bar's fill."""
        root = build("Navigation/BottomCanvasToolbar")
        bar = by_name(root)
        self.assertEqual(rect(bar["Highlight"]), (1.0, 1.0, 511.0, 47.0))
        self.assertTrue(bar["Highlight"].get("stroke") and not bar["Highlight"].get("fill"))
        self.assertFalse(root.get("stroke"))

    def test_the_first_tool_is_selected_and_the_rest_default(self):
        tools = [n for n in walk(build("Navigation/BottomCanvasToolbar")) if n["t"] == "X" and n["name"].startswith("tool-") and n["set"] == "Button/Tool"]
        self.assertEqual([t["variant"] for t in tools], ["State=Selected"] + ["State=Default"] * 5)

    def test_the_ruler_ticks_are_one_vector_per_strip_and_the_labels_are_few_nodes(self):
        rulers = build("Navigation/Rulers")
        vectors = [n for n in walk(rulers) if n["t"] == "V"]
        self.assertEqual([v["name"] for v in vectors], ["Top ticks", "Left ticks"])
        self.assertEqual((vectors[0]["path"].count("M"), vectors[1]["path"].count("M")), (89, 84))
        self.assertLess(len(list(walk(rulers))), 40)

    def test_the_tick_lengths_follow_the_levels(self):
        top = {v["name"]: v for v in walk(build("Navigation/Rulers")) if v["t"] == "V"}["Top ticks"]["path"]
        segs = re.findall(r"M(\d+) ([\d.]+)h1v(\d+)h-1z", top)
        lengths = {int(x): int(v) for x, _, v in segs}
        self.assertEqual((lengths[0], lengths[50], lengths[10]), (6, 14, 3))  # x 309 is a half tick, x 359 a major, x 319 a minor

    def test_the_labels_read_the_values_of_the_capture_and_the_left_ones_are_turned(self):
        rulers = by_name(build("Navigation/Rulers"))
        self.assertEqual([rulers[f"Top label {v}"]["text"] for v in range(0, 45000, 5000)], [str(v) for v in range(0, 45000, 5000)])
        left = [n for n in walk(build("Navigation/Rulers")) if n["name"].startswith("Left label")]
        self.assertEqual((len(left), {n["rot"] for n in left}), (9, {-90}))
        self.assertEqual(left[0]["text"], "-15000")
        self.assertEqual(left[-1]["text"], "25000")



if __name__ == "__main__":
    unittest.main()
