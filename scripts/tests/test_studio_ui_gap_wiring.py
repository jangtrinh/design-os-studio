"""Every entry of the per-screen data modules (`screens/gap_*_data.py`) is placed on the screen of its capture: the right inspector, the layer trees, the Dev, Variables and tool
patches, the agents states, the overlays and their aliases. A data entry nobody places is a state nobody draws, so each module is walked entry by entry against `screens.discover()`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_wiring
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import screens  # noqa: E402
from screens import gap_agents_data as agents  # noqa: E402
from screens import gap_dialogs_data as dialogs  # noqa: E402
from screens import gap_left_dev_data as left_dev  # noqa: E402
from screens import gap_overlays_data as popovers  # noqa: E402
from screens import gap_overlays_data_menus as menus  # noqa: E402
from screens import gap_right_panel_data as right  # noqa: E402
from screens import gap_table_citations as gtc  # noqa: E402
from screens import gap_tool_states_data as tools  # noqa: E402
from screens import gap_variables_present_data as variables_present  # noqa: E402

SPECS = {s["state"]: s for s in screens.discover()}


def placed(state, component):
    return [p for p in SPECS[state]["place"] if p["component"] == component]


def only(state, component):
    found = placed(state, component)
    assert len(found) == 1, (state, component, len(found))
    return found[0]


class DataEntryTest(unittest.TestCase):
    def test_every_state_a_data_module_names_is_a_screen(self):
        tables = (right.RIGHT_PANEL, left_dev.LEFT, left_dev.DEV, variables_present.VARIABLES_PRESENT, tools.TOOLS, agents.AGENTS, popovers.POPOVER, popovers.VECTOR, menus.MENU,
                  dialogs.DIALOG, dialogs.PICKER, dialogs.PALETTE, dialogs.BANNER)
        for table in tables:
            self.assertEqual(sorted(set(table) - set(SPECS)), [])

    def test_the_right_inspector_draws_the_state_and_values_of_every_state_but_the_undrawn_one(self):
        for state, entry in right.RIGHT_PANEL.items():
            panel = only(state, "Panel/RightInspector")
            self.assertEqual((panel["variant"], panel["overrides"]), (entry["variant"], entry["overrides"]), state)
        self.assertEqual(set(right.RIGHT_PANEL) | set(right.UNDRAWN), set(gtc.RIGHT_46) | set(gtc.DEV))
        for state in right.UNDRAWN:
            self.assertIsNone(only(state, "Panel/RightInspector")["variant"], state)

    def test_the_left_panel_draws_the_layer_tree_of_every_state_that_has_one(self):
        for state, entry in left_dev.LEFT.items():
            panel = only(state, "Panel/LeftLayers")
            self.assertEqual(panel["slots"], entry["slots"], state)
            self.assertEqual(panel.get("overrides", []), entry.get("overrides", []), state)
        for state in set(SPECS) - set(left_dev.LEFT):
            for panel in placed(state, "Panel/LeftLayers"):
                self.assertNotIn("slots", panel, f"{state}: a state with no tree data keeps the default rows")

    def test_a_patch_edits_the_placed_instance_and_an_add_places_another(self):
        for data in (left_dev.DEV, variables_present.VARIABLES_PRESENT, tools.TOOLS):
            for state, entry in data.items():
                for component, patch in entry.get("patch", {}).items():
                    for p in [p for p in placed(state, component) if p["name"] == component]:  # the shell's own instance: an add of the same record has another name
                        for key, value in patch.items():
                            if value is None:
                                self.assertNotIn(key, p, (state, component, key))
                            elif key == "variant":
                                self.assertEqual({k: p["variant"][k] for k in value}, value, (state, component))
                            else:
                                self.assertEqual(p[key], value, (state, component, key))
                for add in entry.get("add", []):
                    self.assertIn(add, placed(state, add["component"]), (state, add["name"]))

    def test_the_dev_list_sits_right_after_the_layers_panel_and_the_other_additions_paint_last(self):
        for state in ("g-dev-react", "g-dev-annotate", "g-dev-compare", "micro-dev-boxmodel"):
            names = [p["name"] for p in SPECS[state]["place"]]
            self.assertEqual(names[names.index("Panel/LeftLayers") + 1], "Panel/DevInspect List", state)
        self.assertEqual(SPECS["present-variables"]["place"][-1]["name"], "Window/Variables")
        self.assertEqual(SPECS["micro-components-variables"]["place"][-1]["name"], "Window/Variables")

    def test_one_screens_additions_never_leak_into_the_screen_that_shares_its_place_list(self):
        self.assertEqual([p["component"] for p in SPECS["x-present"]["place"]], ["Bar/Presentation"])
        self.assertEqual(SPECS["x-present"]["place"][0].get("props"), None)

    def test_the_agents_panel_draws_the_state_and_probe_results_of_every_state_that_has_them(self):
        for state, entry in agents.AGENTS.items():
            panel = only(state, "Panel/AgentTerminal")
            self.assertEqual(panel["variant"], entry["variant"], state)
            self.assertEqual((panel.get("props"), panel.get("overrides")), (entry.get("props"), entry.get("overrides")), state)

    def test_every_overlay_entry_is_placed_on_its_screen(self):
        for table in (popovers.POPOVER, popovers.VECTOR):
            for state, entry in table.items():
                self.assertIn(entry, SPECS[state]["place"], state)
        for state, entries in menus.MENU.items():
            for entry in entries:
                self.assertIn(entry, SPECS[state]["place"], state)
        for table, component in screens.gap_wiring.DIALOG_RECORDS:
            for state, entry in table.items():
                p = next(p for p in placed(state, component))
                self.assertEqual((p["variant"], (p["x"], p["y"])), (entry["variant"], entry["xy"]), state)
                for key, value in entry["extra"].items():
                    self.assertEqual(p[key], value, (state, key))

    def test_an_overlay_paints_after_the_shell_regions(self):
        for state in set(popovers.POPOVER) | set(menus.MENU) | set(dialogs.DIALOG):
            names = [p["component"] for p in SPECS[state]["place"]]
            for region in ("Navigation/BottomCanvasToolbar", "Panel/RightInspector"):
                if region in names and state != "j-comments":
                    self.assertLess(names.index(region), max(i for i, n in enumerate(names) if n in ("Popover/Panel", "Menu/Panel", "Dialog/Modal")), (state, region))

    def test_the_named_widgets_the_overlays_draw_are_aliases_of_their_screen(self):
        for table in (popovers.ALIASES, dialogs.ALIASES):
            for state, aliases in table.items():
                self.assertEqual({k: v for k, v in SPECS[state]["aliases"].items() if k in aliases}, aliases, state)
        self.assertEqual(sorted(dialogs.ALIASES["plugin-install"]), ["plugin-install-cancel", "plugin-install-submit", "plugin-url"])

    def test_the_old_stand_in_parts_of_a_state_that_has_a_kind_are_gone(self):
        for state in set(popovers.POPOVER) | {"micro-vector-edit"}:
            for p in SPECS[state]["place"]:
                self.assertNotIn(p["component"], ("Button/Close", "Divider/Hairline", "Radio/Card"), state)
        for state in ("g-dev-compare", "plugin-install"):
            self.assertNotIn("overrides", only(state, "Dialog/Modal"), state)


if __name__ == "__main__":
    unittest.main()
