"""Batch W4C gate data (screens/w4c_gate.py, w4c_overlays_gate.py): every state of the batch declares what the screen gate needs, every cap that is not the editor-default cap carries its cause,
the chrome regions are the rects of the capture's `layout.json`, and the kept rects are the overlays the capture reports. Rects below are read from the 0.7.5 captures when they are available
(the captures are untracked), never from the screens.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_w4c_gate
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import screens  # noqa: E402
import studio_ui_gate_screen_data as gd  # noqa: E402
from evidence import screen_caps as caps  # noqa: E402
from evidence import screen_masks as masks  # noqa: E402
from evidence.capture_dirs import captures_dir  # noqa: E402

CAPTURES = captures_dir(json.loads((HERE.parent / "studio_ui_file/evidence/crops.json").read_text())["captures"], None)
STATES = sorted(s["state"] for s in screens.discover() if s["module"].startswith("w4c_"))
TABLES = gd.load()
CHROME = ("tab_bar", "rail", "left_panel", "right_panel")
NAMES = {"tab_bar": "tab bar", "rail": "rail", "left_panel": "left panel", "right_panel": "right panel"}
DENY = ("untitled", "vsf", "pcp", "olivia", "❖", "_primitives", "color modes", "display 2xl", "dashboards", "swatch base", "logomark", "press logo", "company logo", "screen mockups")


def layout(state):
    path = CAPTURES / f"{state}.layout.json"
    if not path.is_file():
        raise unittest.SkipTest("the 0.7.5 captures are untracked and not in this checkout")
    return json.loads(path.read_text())


class DeclaredTest(unittest.TestCase):
    def test_every_state_of_the_batch_declares_caps_regions_texts_and_content(self):
        for state in STATES:
            for table in ("CAPS", "REGIONS", "TEXTS", "CONTENT", "CAUSES"):
                self.assertIn(state, TABLES[table], f"{state}: {table}")

    def test_the_batch_declares_no_state_that_has_no_screen(self):
        for table in gd.TABLES:
            self.assertLessEqual(set(TABLES[table]) & {"j-cmdk", "web-capture-report", "check-designs-fixed"} - set(STATES), set(), table)
        for state in set(TABLES["CAPS"]) - set(STATES):
            self.assertNotIn(state, ("web-capture-report", "check-designs-fixed"))

    def test_a_cap_exists_for_the_screen_and_for_exactly_the_regions_reported(self):
        for state in STATES:
            self.assertEqual(set(TABLES["CAPS"][state]), {"screen", *TABLES["REGIONS"][state]}, state)

    def test_a_cap_that_is_not_the_editor_default_has_a_cause_and_a_default_cap_has_none(self):
        for state in STATES:
            for region, cap in TABLES["CAPS"][state].items():
                cause = TABLES["CAUSES"][state].get(region)
                if cap == caps.DEFAULT_CAPS.get(region):
                    self.assertIsNone(cause, f"{state}/{region}: a default cap needs no cause")
                else:
                    self.assertTrue(cause and len(cause) > 20, f"{state}/{region}: pinned at {cap} without a cause")

    def test_a_cause_names_a_cap_or_the_worst_tile_and_a_worst_tile_override_has_its_cause(self):
        for state in STATES:
            for key in TABLES["CAUSES"][state]:
                self.assertIn(key, {*TABLES["CAPS"][state], caps.TILE_KEY}, f"{state}: cause for {key}")
            self.assertEqual(state in TABLES["WORST_TILE"], caps.TILE_KEY in TABLES["CAUSES"][state], state)
            if state in TABLES["WORST_TILE"]:
                self.assertGreater(TABLES["WORST_TILE"][state], caps.WORST_TILE, state)

    def test_the_caps_are_the_framework_merge_of_the_gate_data(self):
        for state in STATES:
            self.assertEqual(caps.CAPS[state], TABLES["CAPS"][state])
            self.assertEqual(masks.regions(state), TABLES["REGIONS"][state])
            self.assertEqual(masks.TEXTS[state], TABLES["TEXTS"][state])

    def test_a_geometry_pin_names_a_key_the_capture_has_and_gives_numbers_and_a_cause(self):
        w4c_pins = {s: pins for s, pins in TABLES["GEOMETRY"].items() if s in STATES}
        self.assertEqual(sorted(w4c_pins), ["j-version-history", "k-variables-floating"])
        for state, pins in w4c_pins.items():
            for pin in pins:
                self.assertTrue(pin["cause"] and len(pin["layout"]) == 4 and len(pin["node"]) == 4, pin["key"])
                self.assertIn(pin["key"], layout(state)["named"], f"{state}/{pin['key']}")
                self.assertNotEqual(pin["layout"], pin["node"])


class RectTest(unittest.TestCase):
    def test_every_rect_lies_in_the_window(self):
        for state in STATES:
            rects = list(TABLES["REGIONS"][state].values()) + TABLES["KEEP"].get(state, []) + [r for r, _, _ in TABLES["TEXTS"][state]]
            content = TABLES["CONTENT"][state]
            rects += list(content) if content else []
            for x, y, w, h in rects:
                self.assertTrue(0 <= x and 0 <= y and w >= 0 and h >= 0 and x + w <= 1440 and y + h <= 900, f"{state}: {(x, y, w, h)}")

    def test_the_chrome_regions_are_the_rects_of_the_capture_where_the_screen_draws_them_whole(self):
        whole = {"j-find": CHROME, "j-palette": CHROME, "j-plugins": CHROME, "plugin-install": CHROME, "k-libraries": CHROME, "j-cmdk": CHROME, "k-assets": CHROME,
                 "j-comments": CHROME, "j-version-history": CHROME, "j-check-designs": CHROME}
        for state, keys in whole.items():
            regions = layout(state)["regions"]
            for key in keys:
                self.assertEqual(tuple(regions[key]), tuple(float(v) for v in TABLES["REGIONS"][state][NAMES[key]]), f"{state}: {key}")

    def test_the_variables_window_is_the_rect_of_the_capture(self):
        self.assertEqual(tuple(layout("k-variables")["regions"]["variables_full"]), (49.0, 32.0, 1391.0, 868.0))
        self.assertEqual(TABLES["REGIONS"]["k-variables"]["variables sidebar"], (49, 32, 241, 868))
        self.assertEqual(tuple(layout("k-variables-floating")["overlays"][0]["rect"]), (0.0, 8.0, 642.0, 380.0))
        self.assertEqual(TABLES["REGIONS"]["k-variables-floating"]["floating variables"], (0, 8, 642, 380))

    def test_a_kept_rect_is_an_overlay_of_the_capture_or_the_dialog_the_scrim_dims(self):
        for state, keeps in TABLES["KEEP"].items():
            if state not in STATES:
                continue
            overlays = [tuple(o["rect"]) for o in layout(state)["overlays"]]
            for keep in keeps:
                if (state, keep) == ("j-comments", (1107, 379, 36, 36)):
                    continue  # the comment pin is no overlay of the layout: its rect is the bounding box of its orange disc in the capture picture (x 1107-1142, y 379-414)
                inside = [o for o in overlays if o[0] - 1 <= keep[0] and o[1] - 1 <= keep[1] and keep[0] + keep[2] <= o[0] + o[2] + 1 and keep[1] + keep[3] <= o[1] + o[3] + 1]
                self.assertTrue(inside or state in TABLES["SCRIMS"] or state == "j-version-history", f"{state}: {keep} is not inside an overlay of its capture {overlays}")

    def test_the_presentation_content_is_the_page_and_its_toolbar_rect_is_empty(self):
        for state in ("x-present", "present-variables"):
            content, toolbar = TABLES["CONTENT"][state]
            self.assertEqual(content, (129, 80, 1182, 788))
            self.assertEqual(toolbar[2:], (0, 0))


class ScrimTest(unittest.TestCase):
    def test_the_scrim_is_declared_for_the_three_dimmed_states_and_each_spec_places_it(self):
        self.assertEqual(sorted(s for s in TABLES["SCRIMS"] if s in STATES), ["j-plugins", "k-libraries", "plugin-install"])
        for s in screens.discover():
            if s["state"] in STATES:
                self.assertEqual(bool(s.get("scrim")), s["state"] in TABLES["SCRIMS"], s["name"])
        for state in ("j-plugins", "plugin-install", "k-libraries"):
            self.assertEqual(TABLES["SCRIMS"][state]["variable"], "color/bg-overlay")
            self.assertEqual(layout(state)["overlays"][0]["rect"], [0.0, 0.0, 1440.0, 900.0])


class ContentTest(unittest.TestCase):
    def test_a_replaced_text_names_what_it_replaces_and_the_neutral_text_has_no_kit_name(self):
        for state in STATES:
            for rect, what, neutral in TABLES["TEXTS"][state]:
                self.assertTrue(what and neutral, f"{state}: {rect}")
                self.assertFalse([t for t in DENY if t in (what + neutral).lower()], f"{state}: {neutral}")


if __name__ == "__main__":
    unittest.main()
