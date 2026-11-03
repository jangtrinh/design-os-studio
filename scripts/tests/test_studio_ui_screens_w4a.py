"""The W4a screens (Flow Home & editor shell): the 19 state screens, their names, their instances and their menus. The numbers they get against the captures are the screen gate's, not a unit test's.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screens_w4a
"""
import json
import re
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import screens  # noqa: E402
from screens import editor_design  # noqa: E402
from evidence.capture_dirs import captures_dir  # noqa: E402

REGISTRY = json.loads((REPO / "design/component-registry.json").read_text(encoding="utf-8"))
HOME = ("h-home",)
EDITOR = ("d-window-chrome", "d-menu-main", "d-menu-main-file", "d-menu-shape", "d-menu-zoom", "d-editor-selected", "e-no-selection", "e-layers-selected", "picker-page", "kit-editor",
         "micro-layer-picker", "micro-smart-selection", "micro-selection-corner", "micro-scale-tool", "micro-vector-edit", "micro-shape-controls", "micro-autolayout", "micro-css-grid")
DENY = re.compile(r"untitled|vsf|pcp|olivia|figma|❖", re.I)


def mine():
    return {s["name"]: s for s in screens.discover() if s["module"].startswith("w4a_")}


class NamesTest(unittest.TestCase):
    def test_every_capture_of_the_flow_has_one_screen_under_its_registry_screen(self):
        want = {f"Screen/Home/{c}" for c in HOME} | {f"Screen/EditorDesign/{c}" for c in EDITOR}
        self.assertEqual(set(mine()), want)
        self.assertEqual(len(want), 19)

    def test_the_specs_pass_the_registry_check_and_state_is_the_capture_name(self):
        specs = list(mine().values())
        screens.check_specs(specs, REGISTRY)
        for s in specs:
            self.assertEqual(s["name"].rsplit("/", 1)[1], s["state"])
            self.assertEqual((s["w"], s["h"]), (1440, 900))

    def test_the_first_screen_is_still_there_and_unchanged(self):
        names = [s["name"] for s in screens.discover()]
        self.assertIn("Screen/EditorDesign", names)
        self.assertEqual(len(names), len(set(names)))

    def test_every_state_has_a_capture_with_a_layout_when_the_capture_folder_is_here(self):
        crops = json.loads((HERE.parent / "studio_ui_file/evidence/crops.json").read_text(encoding="utf-8"))
        folder = captures_dir(crops["captures"], None)
        if not (folder / "d-editor-default.png").exists():
            self.skipTest("the 0.7.5 captures are not in this checkout")
        for s in mine().values():
            self.assertTrue((folder / f"{s['state']}.png").exists() and (folder / f"{s['state']}.layout.json").exists(), s["state"])


class ShellTest(unittest.TestCase):
    def test_an_editor_screen_places_the_six_regions_of_the_first_screen(self):
        first = {p["name"]: (p["x"], p["y"], p.get("variant")) for p in editor_design.screens()[0]["place"]}
        for case in ("d-window-chrome", "e-no-selection"):  # the captures that are the first screen's own
            got = {p["name"]: (p["x"], p["y"], p.get("variant")) for p in mine()[f"Screen/EditorDesign/{case}"]["place"]}
            self.assertEqual(got, first, case)
        for case in ("d-editor-selected", "micro-css-grid"):  # the regions sit at the same places and draw the state of their capture
            got = {p["name"]: (p["x"], p["y"]) for p in mine()[f"Screen/EditorDesign/{case}"]["place"]}
            self.assertEqual(got, {n: v[:2] for n, v in first.items()}, case)

    def test_the_editor_screens_draw_the_canvas_colour_and_home_none(self):
        for name, s in mine().items():
            self.assertEqual(bool(s["canvas"]), name.startswith("Screen/EditorDesign/"), name)

    def test_home_turns_the_project_tab_off_and_the_home_tab_on_and_places_the_sidebar_and_main(self):
        s = mine()["Screen/Home/h-home"]
        at = {p["name"]: (p["x"], p["y"]) for p in s["place"]}
        self.assertEqual(at, {"Navigation/TopTabBar": (0, 0), "Panel/HomeSidebar": (0, 32), "Panel/HomeMain": (264, 32)})
        self.assertEqual(s["place"][0]["props"], {"Show project tab": False, "Show Home selected": True})
        self.assertNotIn("overrides", s["place"][0])

    def test_no_screen_carries_a_kit_client_or_library_name(self):
        for name, s in mine().items():
            self.assertIsNone(DENY.search(json.dumps(s, ensure_ascii=False)), name)

    def test_every_w4a_python_file_stays_under_200_lines_and_200_characters_per_line(self):
        paths = sorted((HERE.parent / "studio_ui_file" / "screens").glob("w4a_*.py")) + sorted(HERE.glob("test_studio_ui_screens_w4a*.py"))
        self.assertEqual(len(paths), 7)
        for path in paths:
            lines = path.read_text(encoding="utf-8").splitlines()
            self.assertLess(len(lines), 200, path.name)
            for idx, line in enumerate(lines, 1):
                self.assertLess(len(line), 200, (path.name, idx, len(line)))


def rows(spec, name):
    menu = next(p for p in spec["place"] if p["name"] == name)
    return menu, [r for r in menu["slots"]["Body"]]


class MenuTest(unittest.TestCase):
    def test_the_menus_sit_at_the_overlay_rects_of_the_captures(self):
        want = {"d-menu-main": [("Main menu", 12, 44, 210)], "d-menu-main-file": [("Main menu", 12, 44, 210), ("File submenu", 222, 142, 265)], "d-menu-shape": [("Shape menu", 619, 636, 246)],
                "d-menu-zoom": [("Zoom menu", 1216, 44, 212)], "micro-layer-picker": [("Layer picker", 680, 410, 220)]}
        for case, menus in want.items():
            got = [(p["name"], p["x"], p["y"], p["w"]) for p in mine()[f"Screen/EditorDesign/{case}"]["place"] if p["component"] == "Menu/Panel"]
            self.assertEqual(got, menus, case)

    def test_the_main_menu_has_the_capture_rows_in_order_with_pills_8_px_inside_the_panel(self):
        menu, body = rows(mine()["Screen/EditorDesign/d-menu-main"], "Main menu")
        labels = [r["name"] for r in body if r["component"] == "Menu/Item"]
        self.assertEqual(labels, ["Back to files", "Actions...", "File", "Edit", "View", "Object", "Text", "Arrange", "Vector", "Plugins", "Widgets", "Preferences", "Libraries", "AI balance",
                                  "Help and account"])
        self.assertEqual({r["w"] for r in body if r["component"] == "Menu/Item"}, {194})
        self.assertEqual({r["w"] for r in body if r["component"] == "Menu/Separator"}, {210})
        self.assertEqual(sum(r["component"] == "Menu/Separator" for r in body), 5)

    def test_widgets_is_disabled_and_the_file_row_is_hovered_under_its_submenu(self):
        _, body = rows(mine()["Screen/EditorDesign/d-menu-main"], "Main menu")
        self.assertEqual(next(r for r in body if r["name"] == "Widgets")["variant"]["State"], "Disabled")
        _, body = rows(mine()["Screen/EditorDesign/d-menu-main-file"], "Main menu")
        self.assertEqual(next(r for r in body if r["name"] == "File")["variant"]["State"], "Hover")
        _, sub = rows(mine()["Screen/EditorDesign/d-menu-main-file"], "File submenu")
        self.assertEqual(len([r for r in sub if r["component"] == "Menu/Item"]), 12)
        self.assertEqual({r["w"] for r in sub if r["component"] == "Menu/Item"}, {249})

    def test_a_shortcut_is_set_on_the_nested_hint_and_a_glyph_by_a_swap(self):
        _, body = rows(mine()["Screen/EditorDesign/d-menu-shape"], "Shape menu")
        line = next(r for r in body if r["name"] == "Line")
        self.assertEqual(line["overrides"], [{"path": "Shortcut slot/Shortcut/Keys", "text": "L"}, {"path": "Icon slot/Icon box/Icon", "swap": "Icon/Line"}])
        self.assertEqual(body[0]["variant"]["State"], "Selected")

    def test_the_zoom_menu_starts_with_the_input_field_showing_the_zoom(self):
        _, body = rows(mine()["Screen/EditorDesign/d-menu-zoom"], "Zoom menu")
        self.assertEqual((body[0]["component"], body[0]["overrides"]), ("Input/Field", [{"path": "Value", "text": "2%"}]))
        self.assertEqual([r["name"] for r in body if r.get("variant", {}).get("State") == "Selected"], ["Pixel grid", "Snap to pixel grid", "Layout guides", "Rulers"])


if __name__ == "__main__":
    unittest.main()
