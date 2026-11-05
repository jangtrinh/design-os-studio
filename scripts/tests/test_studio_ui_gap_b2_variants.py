"""The new values of Tree/Row (Depth), Navigation/LeftRail (Tool), Navigation/BottomCanvasToolbar (State), Panel/DevInspect (State) and Window/Variables (Layout), the slots
of Panel/LeftLayers, the tab strip and the Variables toggle of Bar/Presentation, read from the IR the builders return. Pixels are compared in `evidence compare` and `evidence screens`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b2_variants
"""
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from regions_editor_fixtures import ENV, REPO, SPECS, build, by_name, rect  # noqa: E402

REGISTRY = json.loads((REPO / "design/component-registry.json").read_text())
FIRST = {"Tree/Row": ("Depth", "0"), "Navigation/LeftRail": ("Tool", "File"), "Navigation/BottomCanvasToolbar": ("State", "Move"), "Panel/DevInspect": ("State", "Inspect"),
         "Window/Variables": ("Layout", "Full")}


def registry_values(name, axis):
    rec = next(r for r in REGISTRY["components"] if r["name"] == name)
    return [v.split("=")[1] for v in rec["variants"] if v.startswith(axis + "=")]


class FirstValueIsTodaysLookTest(unittest.TestCase):
    def test_each_new_axis_starts_with_the_registrys_first_value(self):
        for name, (axis, first) in FIRST.items():
            self.assertEqual(list(SPECS[name]["axes"][axis]), registry_values(name, axis), name)
            self.assertEqual(SPECS[name]["axes"][axis][0], first, name)

    def test_the_first_depth_and_the_first_tool_draw_what_the_set_drew_before(self):
        self.assertEqual(build("Tree/Row", State="Default")["pad"][3], None)
        rail = by_name(build("Navigation/LeftRail"))
        self.assertEqual(rail["rail-File"]["kids"][0]["variant"], "State=Selected")
        bar = by_name(build("Navigation/BottomCanvasToolbar"))
        self.assertEqual((bar["tool-0"]["variant"], bar["toolbar-actions-btn"]["variant"]), ("State=Selected", "State=Default"))


class TreeDepthTest(unittest.TestCase):
    def test_the_indent_is_a_property_of_the_variant(self):
        step = ENV.num("LAYER_INDENT")
        self.assertEqual(build("Tree/Row", Depth="0")["pad"][3], None)
        self.assertEqual(build("Tree/Row", Depth="1")["pad"][3], ENV.c("LAYER_INDENT"))
        for depth in (2, 3, 4, 5):
            pad = build("Tree/Row", Depth=str(depth))["pad"][3]
            self.assertEqual((pad[0], pad[1]), ("lit", depth * step))

    def test_the_fill_and_the_row_size_do_not_move_with_the_depth(self):
        base = build("Tree/Row", State="Selected", Depth="0")
        for depth in "12345":
            row = build("Tree/Row", State="Selected", Depth=depth)
            self.assertEqual((row["w"], row["h"], row["fill"]), (base["w"], base["h"], base["fill"]))


class RailAndToolbarTest(unittest.TestCase):
    def test_each_tool_selects_its_own_tile_only(self):
        for tool in ("File", "Agents", "Assets", "Variables"):
            rail = by_name(build("Navigation/LeftRail", Tool=tool))
            selected = [n for n in ("File", "Agents", "Assets", "Tools", "Variables", "Plugins") if rail[f"rail-{n}"]["kids"][0]["variant"] == "State=Selected"]
            self.assertEqual(selected, [tool])

    def test_the_toolbar_states_select_the_tiles_the_captures_show(self):
        want = {"Move": ({0}, False), "Comment": ({5}, False), "AI": ({0}, True)}
        for state, (tools, actions) in want.items():
            bar = by_name(build("Navigation/BottomCanvasToolbar", State=state))
            self.assertEqual({i for i in range(6) if bar[f"tool-{i}"]["variant"] == "State=Selected"}, tools, state)
            self.assertEqual(bar["toolbar-actions-btn"]["variant"] == "State=Selected", actions, state)

    def test_the_dev_annotate_dock_selects_annotate_and_measure_not_select(self):
        parts = by_name(build("Navigation/BottomCanvasToolbar", State="DevAnnotate"))
        self.assertEqual({i for i in range(5) if parts[f"tool-{i}"]["variant"] == "State=Selected"}, {1, 2})
        self.assertEqual({n for n in parts if n.startswith("Annotate")}, set())  # a value of the registry, not a hidden part
        self.assertEqual(SPECS["Navigation/BottomCanvasToolbar"].get("props"), None)

    def test_the_dev_dock_is_266_wide_with_the_pill_40_above_it(self):
        root = build("Navigation/BottomCanvasToolbar", State="Dev")
        parts = by_name(root)
        self.assertEqual((root["w"], root["h"]), (266.0, 88.0))
        self.assertEqual((rect(parts["dev-recenter"]), rect(parts["Toolbar"])), ((68.0, 0.0, 131.0, 32.0), (0.0, 40.0, 266.0, 48.0)))
        self.assertEqual([rect(parts[f"tool-{i}"])[0] for i in range(5)], [8.0 + 40 * i for i in range(5)])
        self.assertEqual({i for i in range(5) if parts[f"tool-{i}"]["variant"] == "State=Selected"}, {0, 2})
        self.assertEqual(rect(parts["dev-mode-toggle"]), (222.0, 8.0, 36.0, 32.0))  # 587 + 222 = 809: the capture's dev-mode-toggle


class DevInspectTest(unittest.TestCase):
    def test_the_sizes_of_the_four_states(self):
        for state, size in (("Inspect", (241, 751)), ("Annotate", (241, 751)), ("Component", (241, 751)), ("List", (241, 595))):
            root = build("Panel/DevInspect", State=state)
            self.assertEqual((root["w"], root["h"]), size, state)

    def test_annotate_inserts_the_editor_and_moves_what_follows_down_254(self):
        inspect, annotate = by_name(build("Panel/DevInspect", State="Inspect")), by_name(build("Panel/DevInspect", State="Annotate"))
        self.assertEqual(rect(annotate["Rule Dev links"])[1] - rect(inspect["Rule Dev links"])[1], 254.0)
        self.assertEqual(len([n for n in annotate if n.startswith("Chip ")]), 14)
        self.assertEqual({n for n in annotate if n.startswith("Chip ") and annotate[n].get("stroke")}, {"Chip Width", "Chip Fill"})
        self.assertEqual((rect(annotate["Save"])[1], rect(annotate["Cancel"])[1]), (457, 457))

    def test_component_has_the_playground_and_moves_mcp_162_down(self):
        inspect, comp = by_name(build("Panel/DevInspect", State="Inspect")), by_name(build("Panel/DevInspect", State="Component"))
        self.assertEqual(rect(comp["Rule MCP"])[1] - rect(inspect["Rule MCP"])[1], 162.0)
        self.assertEqual(rect(comp["Playground preview"]), (16, 220, 209, 96))

    def test_the_list_has_fifteen_rows_and_a_hidden_selection(self):
        parts = by_name(build("Panel/DevInspect", State="List"))
        self.assertEqual(len([n for n in parts if n.startswith("Row ")]), 15)
        self.assertTrue(parts["Selected row"]["hidden"])
        self.assertEqual((rect(parts["dev-left-tab-ready"]), rect(parts["left-dev-mark-ready"])), ((16, 62, 83, 24), (10, 25, 220, 24)))


class VariablesAndPresentTest(unittest.TestCase):
    def test_the_three_layouts_have_the_sizes_of_the_captures(self):
        for layout, size in (("Full", (1391, 868)), ("Floating", (642, 380)), ("Present", (264, 836))):
            root = build("Window/Variables", Layout=layout)
            self.assertEqual((root["w"], root["h"]), size, layout)

    def test_the_full_table_default_is_the_captures_rows(self):
        rows = by_name(build("Table/Data"))["Rows"]["kids"]
        self.assertEqual([r["name"].split(" row")[0] for r in rows], ["Group"] * 2 + ["Colour"] * 3 + ["Group"] + ["Colour"] * 11 + ["Group"])
        self.assertEqual(by_name(build("Window/Variables", Layout="Full"))["Table"]["abs"], (241, 49, 521, 778))

    def test_the_floating_table_has_its_own_161_px_mode_column(self):
        parts = by_name(build("Window/Variables", Layout="Floating"))
        self.assertEqual(parts["Table area"]["abs"], (241, 40, 401, 299))
        self.assertEqual(parts["Table area"]["clip"], True)

    def test_the_present_panel_lists_the_live_variables_in_22_px_rows(self):
        parts = by_name(build("Window/Variables", Layout="Present"))
        self.assertEqual(rect(parts["Name 1"])[1] - rect(parts["Heading 48"])[1], 22.0)  # the heading's text sits 6 px into its 26 px row, the first name 2 px into the next
        self.assertEqual(rect(parts["Name 2"])[1] - rect(parts["Name 1"])[1], 22.0)

    def test_the_presentation_bar_has_the_variables_toggle_as_a_show_part(self):
        spec = SPECS["Bar/Presentation"]
        self.assertIn(("Show Variables active", "BOOLEAN", False), [(p["name"], p["type"], p["default"]) for p in spec["props"]])
        part = by_name(build("Bar/Presentation"))["Variables active"]
        self.assertEqual((part["visible_prop"], part["hidden"], part["fill"]), ("Show Variables active", True, ENV.c("BG_SELECTED")))


class LeftPanelAndTabsTest(unittest.TestCase):
    def test_the_left_panel_has_the_two_list_slots_and_the_scrollbar_part(self):
        props = {(p["name"], p["type"]) for p in SPECS["Panel/LeftLayers"]["props"]}
        self.assertEqual(props, {("Pages", "SLOT"), ("Layers", "SLOT"), ("Show pages scrollbar", "BOOLEAN")})

    def test_the_tab_strip_shows_the_three_parts_as_hidden_props(self):
        bar = by_name(build("Navigation/TopTabBar"))
        for part, prop in (("Home selected", "Show Home selected"), ("Dev tile", "Show dev tile"), ("Unsaved dot", "Show unsaved dot")):
            self.assertEqual((bar[part]["visible_prop"], bar[part]["hidden"]), (prop, True), part)
        self.assertEqual(bar["project-tab-0"]["visible_prop"], "Show project tab")


if __name__ == "__main__":
    unittest.main()
