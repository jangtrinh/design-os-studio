"""The W4B screens (Inspector, styles and Dev mode, screens/w4b_*.py): which captures became screens and under which registry screen, that each overlay sits at the capture's rect, that the
gate data is complete (every measured region has a cap, every cap that is not the editor-default one has its cause) and that no kit word is in a screen's texts. Needs no MCP, no Pillow.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screens_w4b
"""
import json
import re
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))
import screen_caps as caps  # noqa: E402
import screen_masks as masks  # noqa: E402
import screens  # noqa: E402
from components import dialogs_compare  # noqa: E402
from regions_editor_fixtures import ENV  # noqa: E402
from screens.gap_dialogs_data import DIALOG  # noqa: E402

REGISTRY = json.loads((REPO / "design/component-registry.json").read_text(encoding="utf-8"))
DENY = re.compile(r"untitled|vsf|pcp|olivia|figma|❖", re.I)
EDITOR_DESIGN = ("f-edit-style-color", "f-effect-styles", "f-effects-menu", "f-picker", "f-picker-gradient", "f-stroke-settings", "f-type-details", "fill-d9", "edit-style-effect",
                 "edit-style-grid", "edit-style-text", "effect-glass", "effect-noise", "effect-texture", "grid-styles", "micro-inspector-math", "micro-typography-multiedit",
                 "micro-components-variables")
EDITOR_DEV = ("micro-dev-boxmodel", "g-dev-annotate", "g-dev-compare", "g-dev-react")
# overlay rects of the captures' layout.json `overlays`, written once here so a screen cannot drift from the capture
OVERLAYS = {"f-edit-style-color": (960, 412, 240, 184), "edit-style-effect": (960, 412, 240, 344), "edit-style-grid": (960, 412, 240, 344), "edit-style-text": (960, 412, 240, 280),
            "f-stroke-settings": (960, 668, 240, 216), "effect-noise": (960, 668, 240, 216), "effect-glass": (960, 636, 240, 248), "effect-texture": (960, 732, 240, 152),
            "f-type-details": (888, 156, 312, 728), "f-prototype-tab": (952, 105, 240, 306), "f-effects-menu": (1264, 571, 168, 289), "grid-styles": (1220, 417, 184, 481),
            "f-effect-styles": (1134, 2, 304, 896), "f-picker": (960, 346, 240, 538), "f-picker-gradient": (960, 332, 240, 552)}


def mine():
    return {s["state"]: s for s in screens.discover() if s["module"].startswith("w4b_")}


def strings(o):
    if isinstance(o, str):
        yield o
    elif isinstance(o, dict):
        for k, v in o.items():
            yield from strings(k)
            yield from strings(v)
    elif isinstance(o, (list, tuple)):
        for v in o:
            yield from strings(v)


class WhichScreensTest(unittest.TestCase):
    def test_each_capture_is_one_screen_under_the_registry_screen_it_shows(self):
        got = {state: s["name"] for state, s in mine().items()}
        want = {c: f"Screen/EditorDesign/{c}" for c in EDITOR_DESIGN} | {c: f"Screen/EditorDev/{c}" for c in EDITOR_DEV} | {"f-prototype-tab": "Screen/EditorPrototype/f-prototype-tab"}
        self.assertEqual(got, want)
        self.assertEqual(len(got), 23)

    def test_the_specs_pass_the_framework_check(self):
        screens.check_specs(list(mine().values()), REGISTRY)

    def test_colour_sheet_and_g_spec_reader_are_not_screens(self):
        """`colour-sheet.png` is a 1170 x 3891 contact sheet of all the captures (no layout.json); `g-spec-reader` is `d-editor-default` with another tab glyph (the reader is not shown)."""
        self.assertFalse({"colour-sheet", "g-spec-reader"} & set(mine()))

    def test_every_screen_is_the_editor_window_at_the_captures_size(self):
        for state, s in mine().items():
            self.assertEqual((s["w"], s["h"]), (1440, 900), state)
            self.assertEqual(s["canvas"][:4], (290, 32, 909, 868), state)
            names = [p["name"] for p in s["place"] if p["name"] != "Panel/DevInspect List"]  # the Dev list covers the Layers part, right after Panel/LeftLayers
            self.assertEqual(names[:6], ["Navigation/TopTabBar", "Navigation/LeftRail", "Panel/LeftLayers", "Navigation/Rulers", "Navigation/BottomCanvasToolbar", "Panel/RightInspector"], state)


class OverlayTest(unittest.TestCase):
    def test_each_overlay_sits_at_the_captures_overlay_rect(self):
        for state, (x, y, w, h) in OVERLAYS.items():
            overlay = mine()[state]["place"][-1]
            self.assertEqual((overlay["x"], overlay["y"]), (x, y), state)
            if overlay["component"] == "Menu/Panel":  # a popover, picker or dialog has the size of its kind; a menu is set to the capture's width
                self.assertEqual(overlay["w"], w, state)

    def test_the_gate_data_reports_the_overlay_at_the_same_rect(self):
        for state, rect in OVERLAYS.items():
            self.assertIn(rect, masks.regions(state).values(), state)
            self.assertIn(rect, masks.KEEP[state], state)

    def test_the_states_without_an_overlay_have_none(self):
        for state in ("fill-d9", "micro-inspector-math", "micro-typography-multiedit", "micro-components-variables", "g-dev-react", "g-dev-annotate", "micro-dev-boxmodel"):
            extra = 2 if state.startswith(("g-dev", "micro-dev")) else 1 if state == "micro-components-variables" else 0  # Dev: the inspector and the list; the floating Variables window
            self.assertEqual(len(mine()[state]["place"]), 6 + extra, state)

    def test_a_popover_is_the_kind_the_capture_shows_and_has_no_stand_in_parts(self):
        p = mine()["f-stroke-settings"]["place"][-1]
        self.assertEqual((p["component"], p["variant"]), ("Popover/Panel", {"State": "Default", "Kind": "Stroke"}))
        self.assertFalse({"props", "overrides", "w", "h"} & set(p))
        td = mine()["f-type-details"]["place"][6:]
        self.assertEqual([(e["component"], e["variant"]["Kind"]) for e in td], [("Popover/Panel", "TypeSettings")])  # 312 wide, with its own close button and hairline

    def test_the_effects_menu_keeps_studios_rows_and_the_menu_is_set_to_the_captures_height(self):
        m = mine()["f-effects-menu"]["place"][6]
        rows = [(e["component"], e.get("props", {}).get("Label"), e["variant"].get("State")) for e in m["slots"]["Body"]]
        self.assertEqual(rows[:7], [("Menu/Item", n, "Default") for n in ("Inner shadow", "Drop shadow", "Layer blur", "Background blur", "Noise", "Texture", "Glass")])
        self.assertEqual(rows[7:], [("Menu/Separator", None, "Default"), ("Menu/Item", "Shader", "Disabled")])
        self.assertEqual((m["w"], m["h"], {e["w"] for e in m["slots"]["Body"] if "w" in e}), (168, 289, {152, 168}))  # rows 16 narrower than the panel, the separator as wide

    def test_the_effect_style_list_has_neutral_names_the_first_selected_and_captions_disabled(self):
        rows = mine()["f-effect-styles"]["place"][6]["slots"]["Body"]
        self.assertEqual(len(rows), 27)
        self.assertEqual([r["props"]["Label"] for r in rows[:2]], ["Group 1", "Style 1"])
        self.assertEqual([r["variant"]["State"] for r in rows[:3]], ["Disabled", "Selected", "Default"])
        self.assertEqual(sum(r["variant"]["State"] == "Disabled" for r in rows), 4)

    def test_the_compare_dialog_has_the_scrim_between_dev_inspect_and_the_dialog(self):
        spec = mine()["g-dev-compare"]
        kids = [(k["kind"], k["name"]) for k in screens.frame_spec(spec, 0)["children"]]
        self.assertEqual(kids[-3:], [("instance", "Panel/DevInspect"), ("rectangle", "Scrim"), ("instance", "Dialog/Modal")])
        self.assertEqual(spec["scrim"]["fill"], "#0000008C")
        self.assertEqual([s for s, v in mine().items() if v.get("scrim")], ["g-dev-compare"])
        self.assertEqual(spec["place"][-1]["variant"], {"Kind": "Compare", "State": "Default"})
        self.assertNotIn("overrides", spec["place"][-1])


class GateDataTest(unittest.TestCase):
    def test_every_measured_region_and_the_screen_have_a_cap_and_no_cap_is_orphaned(self):
        for state in mine():
            self.assertEqual(set(caps.CAPS[state]), {"screen", *masks.regions(state)}, state)

    def test_a_cap_that_is_not_the_editor_default_has_a_cause_and_a_default_cap_has_none(self):
        for state in mine():
            table, causes = caps.CAPS[state], caps.CAUSES[state]
            pinned = {n for n, cap in table.items() if cap != caps.DEFAULT_CAPS.get(n)}
            self.assertEqual({n for n in causes if n != "worst tile"}, pinned, state)
            self.assertTrue(all(len(c) > 40 for c in causes.values()), state)

    def test_a_worst_tile_override_has_its_cause_and_is_a_pin_not_a_calibration(self):
        for state, limit in caps.WORST_TILES.items():
            if state in mine():
                self.assertGreater(limit, caps.WORST_TILE, state)
                self.assertIn("worst tile", caps.CAUSES[state], state)

    def test_a_pin_is_a_measured_mean_to_three_decimals_below_100(self):
        """A pin is a measured mean, not a round number a screen could grow into (the dialog of the compare capture, 91, is the largest gap)."""
        for state in mine():
            for name, cap in caps.CAPS[state].items():
                self.assertEqual(cap, round(cap, 3), (state, name))
                self.assertLess(cap, 100, (state, name))

    def test_the_dev_toolbar_is_its_own_region_and_the_other_toolbars_are_the_editors(self):
        for state in mine():
            want = (587, 840, 266, 48) if state.startswith(("g-dev", "micro-dev")) else (464, 840, 512, 48)
            self.assertEqual(masks.regions(state)["toolbar"], want, state)

    def test_only_the_kit_strings_the_screens_replace_are_left_out_and_none_is_a_kit_word(self):
        for state in mine():
            texts = masks.TEXTS[state]
            self.assertEqual([t for r, w, n in texts if DENY.search(n) or DENY.search(w)], [], state)
            self.assertTrue(all(len(r) == 4 for r, _, _ in texts), state)

    def test_the_compare_dialog_title_is_left_out_where_the_dialog_draws_it_with_a_neutral_frame_name(self):
        title = [k for k in dialogs_compare.modal(ENV)["kids"] if k["name"] == "Title"][0]
        rect, _what, neutral = [t for t in masks.TEXTS["g-dev-compare"] if "compare dialog title" in t[1]][0]
        self.assertEqual(neutral, title["text"])
        self.assertEqual(rect[:2], tuple(a + b for a, b in zip(DIALOG["g-dev-compare"]["xy"], title["abs"][:2])))  # the card's origin plus the title's own offset
        self.assertEqual(rect[3], title["abs"][3])

    def test_no_screen_text_is_a_kit_word(self):
        for state, spec in mine().items():
            self.assertEqual([s for s in strings([spec["place"], spec["name"]]) if DENY.search(s)], [], state)


if __name__ == "__main__":
    unittest.main()
