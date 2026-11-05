"""The per-screen data modules `screens/gap_left_dev_data.py` and `screens/gap_variables_present_data.py`: plain dicts that name real states, real records and
valid variants, and that differ where the captures differ.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b2_data
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import icon_lib  # noqa: E402
import screens  # noqa: E402
from screens import gap_left_dev_data as data  # noqa: E402
from screens import gap_tool_states_data as tools  # noqa: E402
from screens import gap_variables_present_data as var  # noqa: E402
from screens.gap_table_citations import AGENTS, DEV, LEFT_39, RAIL_13  # noqa: E402

REGISTRY = json.loads((HERE.parents[1] / "design/component-registry.json").read_text())
ICONS = icon_lib.IconLib(str(HERE.parents[1]))
SPECS = {s["state"]: s for s in screens.discover()}


class LeftTreesTest(unittest.TestCase):
    def test_every_entry_is_a_left_39_state_with_a_tree_other_than_the_default_rows(self):
        self.assertLessEqual(set(data.LEFT), set(LEFT_39))
        self.assertTrue(set(data.LEFT).isdisjoint(data.SCROLLED))
        for state, entry in data.LEFT.items():
            self.assertNotEqual(data.STATE_TREE[state], "default", state)
            self.assertLessEqual(set(entry["slots"]), {"Pages", "Layers"}, state)

    def test_every_row_is_a_tree_row_with_a_valid_depth_a_known_icon_and_a_neutral_name(self):
        depths = {v.split("=")[1] for r in REGISTRY["components"] if r["name"] == "Tree/Row" for v in r["variants"] if v.startswith("Depth=")}
        for tree in data.TREES:
            rows = data.tree_entries(tree)
            self.assertLessEqual(len(rows), 20, tree)
            for row in rows:
                self.assertEqual((row["component"], row["variant"]["Size"]), ("Tree/Row", "Xs"), tree)
                self.assertIn(row["variant"]["Depth"], depths, tree)
                ICONS.entry(row["props"]["Icon"].split("/")[1])
                self.assertNotRegex(row["props"]["Name"], r"(?i)untitled|figma|vsf|pcp", tree)

    def test_the_selected_rows_of_each_tree_are_the_starred_codes_and_a_chevron_is_a_child_mark(self):
        rows = data.tree_entries("selected")
        self.assertEqual([i for i, r in enumerate(rows) if r["variant"]["State"] == "Selected"], [13])
        self.assertEqual([i for i, r in enumerate(rows) if r["props"].get("Chevron")], [5, 6, 9, 10, 11])
        self.assertEqual([r["variant"]["Depth"] for r in rows[5:15]], ["0", "1", "2", "2", "2", "3", "4", "5", "5", "4"])

    def test_the_version_history_draws_two_pages_and_hides_the_pages_scrollbar(self):
        entry = data.LEFT["j-version-history"]
        self.assertEqual([p["variant"]["State"] for p in entry["slots"]["Pages"]], ["Selected", "Default"])
        self.assertEqual(entry["overrides"], [{"path": "scrollbar-track", "visible": False}])

    def test_states_whose_trees_differ_have_different_rows(self):
        pairs = (("kit-editor", "d-editor-selected"), ("micro-smart-selection", "micro-scale-tool"), ("micro-vector-edit", "micro-shape-controls"), ("f-prototype-tab", "edit-style-text"))
        for a, b in pairs:
            self.assertNotEqual(data.LEFT[a], data.LEFT[b], (a, b))

    def test_the_name_rects_follow_the_depth(self):
        rects = data.LEFT_TEXTS["micro-typography-multiedit"]
        self.assertEqual([r[0][0] for r in rects[:4]], [100, 116, 100, 116])
        self.assertEqual({r[0][1] for r in rects[:2]}, {345, 377})


class DevAndVariablesDataTest(unittest.TestCase):
    def test_the_dev_data_covers_the_four_dev_captures(self):
        self.assertEqual(set(data.DEV), set(DEV))

    def test_every_patch_names_an_instance_the_screen_places_and_every_variant_exists(self):
        records = {r["name"]: {v.split("=")[0]: [] for v in r.get("variants", [])} for r in REGISTRY["components"]}
        values = {r["name"]: [v for v in r.get("variants", [])] for r in REGISTRY["components"]}
        for table in (data.DEV, var.VARIABLES_PRESENT, tools.TOOLS):
            for state, entry in table.items():
                placed = {p["component"] for p in SPECS[state]["place"]}
                for comp, patch in entry.get("patch", {}).items():
                    self.assertIn(comp, placed, (state, comp))
                    for axis, value in (patch.get("variant") or {}).items():
                        self.assertIn(f"{axis}={value}", values[comp], (state, comp))
                for add in entry.get("add", []):
                    self.assertIn(add["component"], records, state)
                    for axis, value in add["variant"].items():
                        self.assertIn(f"{axis}={value}", values[add["component"]], (state, add["component"]))

    def test_the_dev_states_differ_in_the_right_panel_state_and_the_toolbar_state(self):
        inspector = {s: data.DEV[s]["patch"]["Panel/DevInspect"]["variant"]["State"] for s in data.DEV}
        self.assertEqual(inspector, {"g-dev-react": "Inspect", "g-dev-annotate": "Annotate", "g-dev-compare": "Inspect", "micro-dev-boxmodel": "Component"})
        toolbar = {s: data.DEV[s]["patch"]["Navigation/BottomCanvasToolbar"] for s in data.DEV}
        self.assertEqual({s: t["variant"]["State"] for s, t in toolbar.items()}, {"g-dev-react": "Dev", "g-dev-annotate": "DevAnnotate", "g-dev-compare": "Dev", "micro-dev-boxmodel": "Dev"})
        for t in toolbar.values():
            self.assertEqual((t["x"], t["y"], "props" in t), (587, 800, False))

    def test_the_dev_lists_of_the_three_one_item_captures_read_one_and_the_boxmodel_keeps_78(self):
        paths = {"g-dev-react", "g-dev-annotate", "g-dev-compare"}
        for state in paths:
            overrides = data.DEV[state]["add"][0]["overrides"]
            self.assertIn({"path": "left-dev-ready-section/Ready count/Count", "text": "1"}, overrides, state)
            self.assertIn({"path": "Page row/Page count", "text": "1"}, overrides, state)
        self.assertEqual(data.DEV["micro-dev-boxmodel"]["add"][0]["overrides"], [])

    def test_the_floating_states_and_the_present_state_use_the_new_layouts(self):
        self.assertEqual(var.VARIABLES_PRESENT["k-variables-floating"]["patch"]["Window/Variables"]["variant"], {"Layout": "Floating"})
        added = var.VARIABLES_PRESENT["micro-components-variables"]["add"][0]
        self.assertEqual((added["variant"], added["x"], added["y"]), ({"Layout": "Floating"}, 0, 8))
        present = var.VARIABLES_PRESENT["present-variables"]
        self.assertEqual((present["add"][0]["variant"], present["add"][0]["x"], present["add"][0]["y"]), ({"Layout": "Present"}, 1164, 56))
        self.assertEqual(present["patch"]["Bar/Presentation"]["props"], {"Show Variables active": True})

    def test_the_home_tab_hides_the_project_tab_by_its_prop_not_an_override(self):
        patch = var.VARIABLES_PRESENT["h-home"]["patch"]["Navigation/TopTabBar"]
        self.assertEqual(patch["props"], {"Show project tab": False, "Show Home selected": True})
        self.assertIsNone(patch["overrides"])


class ToolStatesTest(unittest.TestCase):
    def test_the_rail_data_covers_the_13_screens_of_row_4_and_names_a_registry_tool(self):
        self.assertEqual(set(tools.RAIL), set(RAIL_13))
        self.assertEqual({tools.RAIL[s] for s in AGENTS}, {"Agents"})
        self.assertEqual((tools.RAIL["k-assets"], tools.RAIL["k-variables"]), ("Assets", "Variables"))

    def test_the_toolbar_data_covers_the_three_screens_of_row_10(self):
        self.assertEqual(tools.TOOLBAR, {"j-comments": "Comment", "j-cmdk": "AI", "j-palette": "AI"})
        self.assertEqual(tools.TOOLS["j-cmdk"]["patch"]["Navigation/BottomCanvasToolbar"]["variant"], {"State": "AI"})


if __name__ == "__main__":
    unittest.main()
