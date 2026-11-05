"""The gate data of batch W4D (screens/w4d_gate.py) against the screens it judges and against the framework's rules.

What is pinned: every state of the batch has every table row the gate needs; the derived caps are the editor-default cap where the measurement meets it and the measurement otherwise, and
every other cap has a cause; the regions, the content mask and the kept overlay rects follow the screen's canvas and overlay instances; the replaced texts are inside the window.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screens_w4d_gate
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import studio_ui_gate_screen_data as gate_data  # noqa: E402
from evidence import screen_caps, screen_masks  # noqa: E402
from screens import w4d_agents, w4d_ai_image  # noqa: E402

DATA = gate_data.load()
SPECS = w4d_agents.screens() + w4d_ai_image.screens()
STATES = [s["state"] for s in SPECS]
GATE = next(path for path in sorted(gate_data.SCREENS.glob("w4d_gate.py")))


def inside(rect, w=1440, h=900):
    x, y, rw, rh = rect
    return x >= 0 and y >= 0 and rw > 0 and rh > 0 and x + rw <= w and y + rh <= h


class TablesTest(unittest.TestCase):
    def test_every_state_has_every_row_the_gate_needs(self):
        from screens import w4d_gate
        for table in ("CAPS", "CAUSES", "TEXTS", "CONTENT", "REGIONS"):
            self.assertEqual(sorted(s for s in DATA[table] if s in STATES), sorted(STATES), table)
        self.assertTrue(set(w4d_gate.WORST_TILE) <= set(STATES))
        self.assertEqual({s: DATA["WORST_TILE"][s] for s in STATES if s in DATA["WORST_TILE"]}, w4d_gate.WORST_TILE)

    def test_the_default_caps_are_the_framework_ones(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location("w4d_gate_copy", GATE)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        self.assertEqual(mod._DEFAULT, screen_caps.DEFAULT_CAPS)

    def test_a_cap_is_the_default_or_a_pinned_number_with_a_cause_and_the_tile_override_has_one(self):
        for state in STATES:
            caps, causes = screen_caps.CAPS[state], screen_caps.CAUSES[state]
            self.assertEqual(set(caps), set(screen_masks.REGIONS[state]) | {"screen"}, state)
            for region, cap in caps.items():
                if cap != screen_caps.DEFAULT_CAPS.get(region):
                    self.assertTrue(causes.get(region), f"{state}: {region} is pinned at {cap} with no cause")
                else:
                    self.assertNotIn(region, causes, f"{state}: {region} is at its default cap and has a cause")
            if state in screen_caps.WORST_TILES:
                self.assertGreater(screen_caps.WORST_TILES[state], screen_caps.WORST_TILE)
                self.assertTrue(causes.get("worst tile"), state)
            else:
                self.assertNotIn("worst tile", causes)

    def test_every_measured_tile_over_the_default_limit_is_pinned_at_its_number(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location("w4d_gate_tiles", GATE)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        over = {s: v for s, v in mod._TILES.items() if v > screen_caps.WORST_TILE}
        self.assertEqual(over, {s: screen_caps.WORST_TILES[s] for s in STATES if s in screen_caps.WORST_TILES})
        self.assertEqual(sorted(mod._TILES), sorted(STATES))

    def test_a_pin_below_its_default_is_never_written_except_the_tab_bar_of_a_screen_that_draws_the_dirty_tab_dot(self):
        for state in STATES:
            for region, cap in screen_caps.CAPS[state].items():
                default = screen_caps.DEFAULT_CAPS.get(region)
                if region == "tab bar" and state.startswith("ai-image-"):
                    self.assertEqual(cap, 0.3, state)  # the measured tab bar mean with the dot drawn, no margin: a missing dot goes over it
                    continue
                self.assertTrue(default is None or cap >= default, f"{state}: {region} {cap} < {default}")


class GeometryTest(unittest.TestCase):
    def test_the_regions_follow_the_canvas_of_the_screen_and_its_overlay(self):
        for s in SPECS:
            regions, (cx, cy, cw, ch) = screen_masks.regions(s["state"]), s["canvas"][:4]
            self.assertEqual(regions["left panel"], (49, 32, cx - 49, 868), s["state"])
            self.assertEqual((regions["ruler top strip"], regions["ruler left strip"]), ((cx + 20, 32, cw - 20, 20), (cx, 52, 20, 848)), s["state"])
            self.assertTrue(all(inside(r) for r in regions.values()), s["state"])
            overlay = [p for p in s["place"] if p["component"] in ("Popover/Panel", "Menu/Panel")]
            self.assertEqual(regions.get("overlay", (None, None))[:2], (overlay[0]["x"], overlay[0]["y"]) if overlay else (None, None), s["state"])  # its size is the kind's own

    def test_the_content_mask_is_the_canvas_below_and_right_of_the_strips_and_the_toolbar_stays_in(self):
        for s in SPECS:
            (x, y, w, h), toolbar = screen_masks.CONTENT[s["state"]]
            self.assertEqual((x, y, w, h), (s["canvas"][0] + 20, s["canvas"][1] + 20, s["canvas"][2] - 20, s["canvas"][3] - 20), s["state"])
            self.assertEqual((toolbar[1], toolbar[3]), (screen_masks.TOOLBAR[1], screen_masks.TOOLBAR[3]), s["state"])
            self.assertEqual((toolbar[0], toolbar[0] + toolbar[2]), (max(screen_masks.TOOLBAR[0], x), screen_masks.TOOLBAR[0] + screen_masks.TOOLBAR[2]), s["state"])
            self.assertGreaterEqual(toolbar[0], x, "the toolbar is clipped to the content rect")

    def test_no_content_exclusion_covers_a_pixel_of_a_ruler_strip(self):
        for s in SPECS:
            texts = [r for r, _, _ in screen_masks.TEXTS[s["state"]]]
            content = [r for r in screen_masks.exclusions(s["state"]) if r not in texts]
            for name in ("ruler top strip", "ruler left strip"):
                sx, sy, sw, sh = screen_masks.REGIONS[s["state"]][name]
                for x, y, w, h in content:
                    overlap = min(x + w, sx + sw) > max(x, sx) and min(y + h, sy + sh) > max(y, sy)
                    self.assertFalse(overlap, f"{s['state']}: an exclusion {(x, y, w, h)} covers the {name} {(sx, sy, sw, sh)}")

    def test_a_kept_overlay_lies_inside_the_content_and_is_the_overlay_region(self):
        kept = sorted(s for s in screen_masks.KEEP if s in STATES)
        self.assertEqual(kept, ["ai-image-erase", "ai-image-expand", "i-agent-permissions"])
        for state in kept:
            rects = screen_masks.KEEP[state]
            (x, y, w, h), _ = screen_masks.CONTENT[state]
            for kx, ky, kw, kh in rects:
                self.assertTrue(x <= kx and y <= ky and kx + kw <= x + w and ky + kh <= y + h, state)
            self.assertEqual(rects, [screen_masks.REGIONS[state]["overlay"]])
        for state in ("agents-skills", "ai-image-menu"):
            self.assertNotIn(state, screen_masks.KEEP, "this overlay lies over a panel the content mask never covers")

    def test_a_kept_rect_is_measured_and_a_replaced_text_never_is(self):
        rects = screen_masks.exclusions("i-agent-permissions")
        self.assertFalse(any(r[0] <= 700 < r[0] + r[2] and r[1] <= 400 < r[1] + r[3] for r in rects), "the permissions popover is measured")
        self.assertTrue(any(r[0] <= 700 < r[0] + r[2] and r[1] <= 200 < r[1] + r[3] for r in rects), "the canvas around it is left out")


class TextsTest(unittest.TestCase):
    def test_every_replaced_text_is_inside_the_window_and_carries_what_and_the_neutral_text(self):
        for state in STATES:
            for rect, what, neutral in screen_masks.TEXTS[state]:
                self.assertTrue(inside(rect), f"{state}: {what} {rect}")
                self.assertTrue(what and neutral, state)

    def test_the_project_tab_title_is_replaced_on_every_screen_and_the_layer_names_only_where_the_capture_has_a_tree(self):
        for state in STATES:
            whats = [w for _, w, _ in screen_masks.TEXTS[state]]
            self.assertIn("project tab title", whats)
            self.assertEqual("layer name" in whats, state.startswith("ai-image"), state)
            self.assertEqual("selection chip text" in whats, not state.startswith("ai-image"), state)


if __name__ == "__main__":
    unittest.main()
