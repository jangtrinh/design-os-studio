"""Batch W4C: the screens of the 0.7.5 captures of panels, dialogs, variables and presentation (screens/w4c_*.py).

Each screen is named `Screen/<registry screen>/<capture>`, places the shell regions at the rects `Screen/EditorDesign` uses, and places its overlay or panel region at the rect its capture's
`layout.json` gives (`overlays`, `regions` and the named rects of the painter: the values below were read from the 0.7.5 captures, not from the screens). Two captures repeat another state
pixel for pixel and are not built.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_w4c_screens
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import screens  # noqa: E402
from evidence.capture_dirs import captures_dir  # noqa: E402

REGISTRY = json.loads((REPO / "design/component-registry.json").read_text(encoding="utf-8"))
SHELL = {"Navigation/TopTabBar": (0, 0), "Navigation/LeftRail": (0, 32), "Panel/LeftLayers": (49, 32), "Navigation/Rulers": (290, 32), "Navigation/BottomCanvasToolbar": (464, 840),
         "Panel/RightInspector": (1199, 32)}
# case -> (registry screen, the region that is not the shell and its rect, the shell regions the capture replaces or leaves out)
EXPECT = {
    "j-check-designs": ("EditorDesign", ("Panel/CheckDesigns", (867, 44)), ()),
    "j-cmdk": ("EditorDesign", ("Dialog/CommandPalette", (500, 512)), ()),
    "j-comments": ("EditorDesign", ("Panel/Comments", (1199, 32)), ("Panel/RightInspector",)),
    "j-find": ("EditorDesign", ("Panel/FindReplace", (49, 305)), ()),
    "j-palette": ("EditorDesign", ("Dialog/CommandPalette", (460, 438)), ()),
    "j-plugins": ("EditorDesign", ("Dialog/Modal", (440, 230)), ()),
    "j-version-history": ("EditorDesign", ("Panel/VersionHistory", (1199, 32)), ("Panel/RightInspector",)),
    "k-assets": ("EditorDesign", ("Panel/Assets", (49, 32)), ("Panel/LeftLayers",)),
    "k-libraries": ("EditorDesign", ("Dialog/Modal", (440, 130)), ()),
    "k-variables": ("EditorDesign", ("Window/Variables", (49, 32)), ("Panel/LeftLayers", "Navigation/Rulers", "Navigation/BottomCanvasToolbar", "Panel/RightInspector")),
    "k-variables-floating": ("EditorDesign", ("Window/Variables", (0, 8)), ()),
    "plugin-install": ("EditorDesign", ("Dialog/Modal", (440, 230)), ()),
    "present-variables": ("Present", ("Bar/Presentation", (0, 0)), ()),
    "x-present": ("Present", ("Bar/Presentation", (0, 0)), ()),
}
NOT_BUILT = ("web-capture-report", "check-designs-fixed")
ABOVE = {"j-comments": ["Popover/Panel"], "j-version-history": ["Banner/Inline"], "present-variables": ["Window/Variables"]}  # what a capture draws over its main region
VARIANT = {("j-cmdk", "Dialog/CommandPalette"): {"Kind": "CmdK", "State": "Default"}, ("j-palette", "Dialog/CommandPalette"): {"Kind": "Actions", "State": "Default"},
           ("j-plugins", "Dialog/Modal"): {"Kind": "Plugins", "State": "Default"}, ("plugin-install", "Dialog/Modal"): {"Kind": "InstallPlugin", "State": "Default"},
           ("k-libraries", "Dialog/Modal"): {"Kind": "Libraries", "State": "Default"}}
CAPTURES = captures_dir(json.loads((HERE.parent / "studio_ui_file/evidence/crops.json").read_text())["captures"], None)


def mine():
    return {s["state"]: s for s in screens.discover() if s["module"].startswith("w4c_")}


class NamingTest(unittest.TestCase):
    def test_every_capture_of_the_batch_has_one_screen_named_after_the_registry_screen_it_shows(self):
        found = mine()
        self.assertEqual(sorted(found), sorted(EXPECT))
        records = {r["name"] for r in REGISTRY["components"] if r.get("category") == "screen"}
        for case, (flow, _, _) in EXPECT.items():
            self.assertEqual(found[case]["name"], f"Screen/{flow}/{case}")
            self.assertIn(f"Screen/{flow}", records)

    def test_the_captures_that_repeat_another_state_are_not_built(self):
        self.assertFalse(set(NOT_BUILT) & set(mine()))

    def test_those_captures_are_pixel_for_pixel_the_state_they_repeat(self):
        try:
            from PIL import Image, ImageChops
        except ImportError:
            self.skipTest("Pillow is not installed")
        if not CAPTURES.is_dir():
            self.skipTest("the 0.7.5 captures are untracked and not in this checkout")
        for repeat, original in (("web-capture-report", "d-editor-default"), ("check-designs-fixed", "j-check-designs")):
            a, b = Image.open(CAPTURES / f"{repeat}.png").convert("RGB"), Image.open(CAPTURES / f"{original}.png").convert("RGB")
            self.assertIsNone(ImageChops.difference(a, b).getbbox(), repeat)


class PlacementTest(unittest.TestCase):
    def test_the_shell_regions_sit_where_the_editor_screen_puts_them_and_a_region_a_panel_replaces_is_left_out(self):
        for case, spec in mine().items():
            at = {p["name"]: (p["x"], p["y"]) for p in spec["place"]}
            _, _, left_out = EXPECT[case]
            for name, xy in SHELL.items():
                if name in left_out:
                    self.assertNotIn(name, at, f"{case}: {name} is replaced by the panel the capture shows")
                elif case not in ("x-present", "present-variables"):
                    self.assertEqual(at.get(name), xy, f"{case}: {name}")

    def test_the_overlay_or_panel_is_placed_at_the_rect_of_the_capture_after_the_shell(self):
        for case, spec in mine().items():
            name, xy = EXPECT[case][1]
            placed = [p for p in spec["place"] if p["component"] == name]
            self.assertEqual([(p["x"], p["y"]) for p in placed], [xy], case)
            above = [p["component"] for p in spec["place"][spec["place"].index(placed[0]) + 1:]]
            self.assertEqual(above, ABOVE.get(case, []), f"{case}: {name} paints over the shell, and only the capture's own overlays paint over it")

    def test_a_variant_is_one_the_registry_lists_for_the_record(self):
        known = {r["name"]: r.get("variants", []) for r in REGISTRY["components"]}
        for case, spec in mine().items():
            for p in spec["place"]:
                self.assertIn(p["component"], known, f"{case}: {p['component']} is not a registry record")
                for axis, value in (p["variant"] or {}).items():
                    self.assertIn(f"{axis}={value}", known[p["component"]], f"{case}: {p['name']}")
                if (case, p["component"]) in VARIANT:
                    self.assertEqual(p["variant"], VARIANT[(case, p["component"])], case)

    def test_find_draws_over_the_layers_list_it_replaces(self):
        order = [p["component"] for p in mine()["j-find"]["place"]]
        self.assertLess(order.index("Panel/LeftLayers"), order.index("Panel/FindReplace"))

    def test_the_floating_window_is_the_floating_layout_at_the_capture_overlays_origin(self):
        p = [p for p in mine()["k-variables-floating"]["place"] if p["component"] == "Window/Variables"][0]
        self.assertEqual((p["x"], p["y"], p["variant"]), (0, 8, {"Layout": "Floating"}), "k-variables-floating.layout.json overlay Middle/0 (642 x 380: the layout's own size)")
        self.assertFalse({"w", "h"} & set(p))

    def test_the_presentation_screens_draw_the_page_colour_under_the_bar_and_nothing_else_local(self):
        for case in ("x-present", "present-variables"):
            spec = mine()[case]
            self.assertEqual(spec["canvas"][:4], (0, 0, 1440, 900))
            self.assertEqual([p["component"] for p in spec["place"]], ["Bar/Presentation"] + (["Window/Variables"] if case == "present-variables" else []))

    def test_the_dialogs_over_a_dimmed_window_carry_the_scrim_the_capture_shows(self):
        for case in ("j-plugins", "plugin-install", "k-libraries"):
            spec = mine()[case]
            order = [p["name"] for p in spec["place"]]
            self.assertEqual(order.index(spec["scrim"]["above"]), len(order) - 2, f"{case}: the scrim is painted right above the shell, so below the dialog")
            self.assertEqual(spec["scrim"], {"fill": "var:Semantic/color/bg-overlay", "above": "Panel/RightInspector"}, f"{case}.layout.json overlay Foreground/0 is the whole window")
        for case in set(EXPECT) - {"j-plugins", "plugin-install", "k-libraries"}:
            self.assertFalse(mine()[case].get("scrim"), case)
        self.assertNotIn("overrides", mine()["plugin-install"]["place"][-1])  # the install form is the kind's own, not the Plugins dialog with another title


if __name__ == "__main__":
    unittest.main()
