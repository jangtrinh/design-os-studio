"""Bar/Presentation (components/regions_home_present.py) read from the IR the builder returns against the real Semantic plan. The rects are the 0.7.5 captures `x-present` and `present-variables`
(`*.layout.json` named rects of a 1440 x 900 window: the bar sits at its top-left corner, so they are the region's own); the Rust is v0.7.5 `present.rs` `show` (:405-500). Structure only: pixels are
compared in `evidence compare`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_home_present
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import icon_lib  # noqa: E402
import text_styles_plan  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
from components.env import Env  # noqa: E402
from rest_ir import pads, walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPECS = {s["name"]: s for s in components.discover()}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def part(node, name):
    found = [k for k in walk(node) if k["name"] == name]
    assert len(found) == 1, (name, len(found))
    return found[0]


class PresentBarTest(unittest.TestCase):
    def setUp(self):
        self.spec = SPECS["Bar/Presentation"]
        self.root = self.spec["build"]({}, ENV)

    def test_it_is_a_plain_region_record_the_width_of_the_window_and_48_high_with_no_fill(self):
        """`PRESENT_BAR_HEIGHT` 48 (:442). The page behind it is `page_bg` (:439), the document's own page colour or `PRESENT_BG`: the screen's, so the bar itself has no fill."""
        self.assertEqual(self.spec["axes"], {})
        self.assertEqual((self.root["w"], self.root["h"], self.root.get("fill")), (1440, ENV.num("PRESENT_BAR_HEIGHT"), None))

    def test_the_hairline_is_the_nested_divider_on_the_last_pixel_row(self):
        """:499 `widgets::hline(&p, bar.left(), bar.right(), bar.bottom() - 1.0, BORDER)`; the only nested set (the buttons are `button_bg` ghosts of 24 px, no Button set is that size)."""
        self.assertEqual([(k["name"], k["set"], k["abs"]) for k in walk(self.root) if k["t"] == "X"], [("Rule", "Divider/Hairline", (0, 47, 1440, 1))])
        self.assertEqual(self.spec["uses"], ["Divider/Hairline"])

    def test_every_named_rect_of_the_capture_is_the_frame_of_the_same_part(self):
        """present-flow 16, 12, 200, 24; present-restart 224, 12, 76, 24; present-vars-toggle 308, 12, 76, 24; present-fullscreen 1296, 12, 96, 24; present-close 1400, 8, 32, 32."""
        want = {"Flow": (16, 12, 200, 24), "Restart": (224, 12, 76, 24), "Variables": (308, 12, 76, 24), "Fullscreen": (1296, 12, 96, 24), "Close": (1400, 8, 32, 32)}
        self.assertEqual({name: tuple(part(self.root, name)["abs"]) for name in want}, want)

    def test_the_flow_name_is_a_secondary_background_pill_with_the_label_8_in(self):
        """:452-456 (no flows in the file: the plain branch): `BG_SECONDARY` at `RADIUS_CONTROL`, the name at `dd.left() + 8` in `font_body` and `TEXT`."""
        flow = part(self.root, "Flow")
        self.assertEqual((flow["fill"], flow["radius"], pads(flow)[3][:2]), ("color/bg-secondary", "radius/control", ("lit", 8.0)))
        label = part(flow, "Label")
        self.assertEqual((label["style"], label["color"], label["prop"]), ("Body / Regular", "color/text", "Flow name"))

    def test_the_buttons_are_ghost_frames_with_a_centred_body_label(self):
        """:461-486 `button_bg` (the icon look: no fill at rest) and a centred `font_body` label: `TEXT`, except the variables toggle's `TEXT_SECONDARY` while it is off (:476)."""
        for name, text, color in (("Restart", "Restart", "color/text"), ("Variables", "Variables", "color/text-secondary"), ("Fullscreen", "Fullscreen", "color/text")):
            button = part(self.root, name)
            self.assertEqual((button.get("fill"), button["radius"], button["align"]), (None, "radius/control", ("center", "center")), name)
            label = part(button, "Label")
            self.assertEqual((label["text"], label["style"], label["color"]), (text, "Body / Regular", color), name)

    def test_the_close_button_draws_its_cross_in_the_icon_colour(self):
        """:490-493 `icon_close(&p, cb.center(), theme::ICON)`: an own painter (two 1.5 px strokes of 10 px), not Button/Close's quaternary glyph; the Phosphor cross stands in."""
        cross = part(part(self.root, "Close"), "Cross")
        self.assertEqual((cross["icon"], cross["color"]), ("Close", "color/icon"))

    def test_the_title_is_centred_on_the_bar_in_secondary_text(self):
        """:497-498 `Align2::CENTER_CENTER` at `bar.center()`, `font_body`, `TEXT_SECONDARY`."""
        title = part(self.root, "Title")
        self.assertEqual((title["abs"], title["align"]), ((0, 0, 1440, 48), ("center", "center")))
        label = part(title, "Label")
        self.assertEqual((label["style"], label["color"], label["prop"]), ("Body / Regular", "color/text-secondary", "Title"))

    def test_the_frame_name_default_is_neutral_and_every_text_uses_an_existing_style_and_a_semantic_colour(self):
        names = {s["name"] for s in PLAN["semantic"]}
        for k in walk(self.root):
            if k["t"] == "T":
                self.assertIn(k["style"], STYLES)
                self.assertIn(k["color"], names)
        self.assertEqual({part(self.root, "Flow")["kids"][0]["text"], part(self.root, "Title")["kids"][0]["text"]}, {"Frame 1"})
        self.assertEqual([(p["name"], p["type"]) for p in self.spec["props"]], [("Flow name", "TEXT"), ("Title", "TEXT"), ("Show Variables active", "BOOLEAN")])


if __name__ == "__main__":
    unittest.main()
