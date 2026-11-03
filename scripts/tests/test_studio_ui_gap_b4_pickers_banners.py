"""Popover/ColorPicker Kind=Fill, Page, Gradient (components/color_picker*.py), Banner/Inline (components/banners.py) and Card/Approval (components/cards_agents.py), read from the IR the builders
return against the real Semantic plan and the real registry.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b4_pickers_banners
"""
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio_ui_regions_fixtures as fx  # noqa: E402
import components  # noqa: E402
from rest_ir import walk  # noqa: E402

REGISTRY = json.loads((fx.REPO / "design/component-registry.json").read_text(encoding="utf-8"))
PICKER, BANNER, CARD = fx.SPECS["Popover/ColorPicker"], fx.SPECS["Banner/Inline"], fx.SPECS["Card/Approval"]


def picker(kind):
    return PICKER["build"]({"Kind": kind}, fx.ENV)


def banner(tone):
    return BANNER["build"]({"Tone": tone}, fx.ENV)


def names(node):
    return [k["name"] for k in walk(node)]


class PickerTest(unittest.TestCase):
    def test_fill_is_the_first_kind_and_the_axes_are_the_registrys(self):
        components.check_specs([PICKER, BANNER, CARD], REGISTRY)
        self.assertEqual(PICKER["axes"], {"Kind": ["Fill", "Page", "Gradient"]})

    def test_the_heights_are_the_painters_panel_heights(self):
        self.assertEqual([picker(k)["h"] for k in ("Fill", "Page", "Gradient")], [538, 498, 552], "picker.rs panel_height: PAINT_ROW_H 40 less for a page, STOPS_H 40 for CONTRAST_BAR_H 26")

    def test_page_is_a_plain_title_without_tabs_add_style_or_the_paint_row(self):
        p = picker("Page")
        for gone in ("Custom tab", "Libraries tab", "Add style", "Remove paint", "Blend", "Mode 0", "Rule 79"):
            self.assertNotIn(gone, names(p))
        self.assertEqual((fx.part(p, "Title")["abs"][:2], fx.texts(fx.part(p, "Title"))), ((16, 8), ["Custom"]))
        self.assertEqual(fx.part(p, "Close")["abs"], (208, 8, 24, 24))

    def test_page_moves_everything_under_the_header_up_by_the_paint_row(self):
        fill, page = picker("Fill"), picker("Page")
        for name, y in (("Contrast bar", 86), ("SV square", 122), ("Hue strip", 343), ("Alpha strip", 371), ("Format", 397), ("Hex field", 397), ("Source", 450), ("Swatch 0", 482)):
            self.assertEqual((fx.part(fill, name)["abs"][1], fx.part(page, name)["abs"][1]), (y, y - 40), name)

    def test_page_draws_the_page_colour_f5f5f5_with_its_contrast(self):
        p = picker("Page")
        self.assertIn("15.29 : 1", fx.texts(p))
        self.assertIn("F5F5F5", fx.texts(p))
        self.assertEqual(fx.part(p, "SV thumb")["abs"][:2], (9.0, 83.0), "the thumb at saturation 0 and value 245 / 255, 7 px from its corner")

    def test_gradient_has_the_stops_row_in_the_place_of_the_contrast_row(self):
        g = picker("Gradient")
        self.assertNotIn("Contrast bar", names(g))
        self.assertEqual(fx.part(g, "Stops bar")["abs"], (16, 92, 152, 16))
        self.assertEqual([fx.part(g, n)["abs"] for n in ("Flip", "Rotate")], [(172, 88, 24, 24), (200, 88, 24, 24)])
        self.assertEqual((fx.part(g, "Flip")["props"], fx.part(g, "Rotate")["props"]), ({"Icon": "FlipH"}, {"Icon": "Rotate90"}))
        self.assertEqual(fx.part(g, "SV square")["abs"][1], 122 + 14)

    def test_gradient_marks_the_linear_button_active_and_stop_one_selected(self):
        g = picker("Gradient")
        filled = [k["name"] for k in walk(g) if k["name"].startswith("Mode ") and k.get("fill")]
        self.assertEqual(filled, ["Mode 1"])
        self.assertIn("Stop 1 ring", names(g))
        self.assertNotIn("Stop 2 ring", names(g))

    def test_fill_keeps_its_nodes_and_rects_unchanged(self):
        f = picker("Fill")
        self.assertEqual((f["w"], f["h"]), (240, 538))
        self.assertEqual(fx.part(f, "SV square")["abs"], (16, 122, 208, 208))
        self.assertEqual(fx.part(f, "Custom tab")["abs"][:2], (8, 8))
        self.assertIn("16.67 : 1", fx.texts(f))


class BannerTest(unittest.TestCase):
    def test_neutral_is_the_first_tone_and_success_takes_its_look(self):
        self.assertEqual(BANNER["axes"], {"Tone": ["Neutral", "Success", "Warning", "Error"]})
        self.assertEqual(banner("Success"), banner("Neutral"), "no painter draws Success: rule (c), the Neutral look")

    def test_neutral_is_the_history_bar_909_by_40(self):
        b = banner("Neutral")
        self.assertEqual((b["w"], b["h"], b["fill"], b["stroke"]), (909.0, 40.0, "color/bg-secondary", "color/border"))
        self.assertEqual([(k["set"], k["variant"], k["props"]["Label"]) for k in b["kids"] if k["t"] == "X"],
                         [("Button/Primary", "Size=Xs, State=Default", "Restore this version"), ("Button/Secondary", "Size=Xs, State=Default", "Back to current")])

    def test_warning_and_error_are_the_failure_banner_48_high_in_their_tones(self):
        for tone, fill, edge in (("Warning", "color/bg-warning-primary", "color/badge-warning-border"), ("Error", "color/bg-error-primary", "color/badge-error-border")):
            b = banner(tone)
            self.assertEqual((b["w"], b["h"], b["fill"], b["stroke"]), (401.0, 48.0, fill, edge), tone)
        self.assertEqual([k["set"] for k in walk(banner("Error")) if k["t"] == "X"][0], "Status/Dot")
        self.assertEqual([k["t"] for k in walk(banner("Warning")) if k["name"] == "Dot"], ["E"], "Status/Dot has no Warning tone: a local disc")


class CardApprovalTest(unittest.TestCase):
    def setUp(self):
        self.c = CARD["build"]({}, fx.ENV)

    def test_it_is_a_392_wide_hugging_card_with_the_claude_default(self):
        c = self.c
        self.assertEqual((c["w"], c["sy"], c["fill"], c["radius"], c["gap"]), (392.0, "hug", "color/bg-tertiary", "radius/frame", "space/row"))
        self.assertEqual(fx.texts(fx.part(c, "Header")), ["CLAUDE", "Claude Code wants to run Bash", "1 of 3"])
        self.assertEqual(fx.texts(fx.part(c, "Summary")), ["cargo test -p designos-app --bin designos"])

    def test_the_cli_dependent_parts_are_show_properties_with_the_claude_defaults(self):
        props = {p["name"]: p["default"] for p in CARD["props"] if p["type"] == "BOOLEAN"}
        self.assertEqual(props, {"Show summary": True, "Show actions": True, "Show prompt": False, "Show count": False})
        self.assertEqual({k["name"]: k["visible_prop"] for k in walk(self.c) if k.get("visible_prop")},
                         {"Count": "Show count", "Summary": "Show summary", "Actions": "Show actions", "Prompt row": "Show prompt"})


if __name__ == "__main__":
    unittest.main()
