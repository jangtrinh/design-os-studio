"""Panel/HomeMain (components/regions_home_main.py) read from the IR the builder returns against the real Semantic plan. The rects are the 0.7.5 capture `h-home` (`h-home.layout.json`
named rects of a 1440 x 900 window whose main area starts at (264, 32)), so every rect here is the capture's minus (264, 32); the Rust is v0.7.5 `home/mod.rs` `mod main_view`
(`show` :1292-1430, `files_view` :2413-). Structure only: pixels are compared in `evidence compare`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_home_main
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


class HomeMainTest(unittest.TestCase):
    def setUp(self):
        self.spec = SPECS["Panel/HomeMain"]
        self.root = self.spec["build"]({}, ENV)

    def test_it_is_a_plain_region_record_with_a_content_slot_and_the_components_it_reuses(self):
        """No registry variants (one component); the content area differs per section (the card grid, the empty state, a list), so it is a native Slot (S5)."""
        self.assertEqual(self.spec["axes"], {})
        self.assertEqual([(p["name"], p["type"]) for p in self.spec["props"]], [("Content", "SLOT")])
        self.assertEqual(sorted(self.spec["uses"]), ["Button/Icon", "Button/Primary", "Button/Secondary", "Card/FileThumbnail", "Divider/Hairline"])
        self.assertEqual({k["set"] for k in walk(self.root) if k["t"] == "X"}, set(self.spec["uses"]))

    def test_the_region_is_the_home_main_area_1176_by_868_on_the_page_colour(self):
        """The window less the 264 px sidebar (h-home.layout.json regions.home 0, 32, 1440, 868); `p.rect_filled(rect, 0.0, BG)` (:1294). A head and sub-header of 111 (`HOME_HEADER_H` 64 +
        4 + `CONTROL_SM` + 10 + the hairline), then the content below it."""
        self.assertEqual((self.root["w"], self.root["h"], self.root["fill"], self.root["dir"]), (1176, 868, "color/bg", "V"))
        top, content = self.root["kids"]
        self.assertEqual((top["name"], top["h"], top["sy"]), ("Top", 111, "fixed"))
        self.assertEqual((content["name"], content["slot"], content["sx"], content["sy"]), ("Content", "Content", "fill", "fill"))

    def test_the_title_is_the_breadcrumb_text_centred_in_the_64_px_header(self):
        """`breadcrumb` (:1472-1530): `font_text(FONT_SIZE_TEXT_XL, Semibold)` in `TEXT` (the last crumb), at (pad, head.center().y) = (32, 32): a 30 px line box at y 17 (home-crumb-0 296, 49,
        78.25, 30). It is `Text xl / Semibold`, the style that names `FONT_SIZE_TEXT_XL`."""
        crumb = part(self.root, "Title")
        self.assertEqual((crumb["text"], crumb["style"], crumb["color"], crumb["abs"][:2]), ("Recents", "Text xl / Semibold", "color/text", (32, 17)))
        self.assertEqual(crumb["abs"][3], ENV.num("LINE_TEXT_XL"))

    def test_the_three_buttons_are_right_aligned_36_px_copies_8_apart_ending_32_from_the_edge(self):
        """`+ Design file` Primary, `+ FigJam board` and `Import` Secondary, each `button_width` wide = label + 2 * SPACE_GROUP, `gap` 8 between (:1310, :1343-1360), `right = rect.right() - pad` =
        1144, centred on the 64 px header: x 1035.25 / 898 / 821.5, y 14 in the capture (home-new-file 1299.25, 46; home-new-board 1162; home-import 1085.5)."""
        group = part(self.root, "Actions")
        self.assertEqual((group["abs"], group["dir"], group["align"][0]), ((821.5, 14, 322.5, 36), "H", "end"))
        self.assertEqual(group["gap"][:2], ("lit", 8.0))
        got = [(k["name"], k["set"], k["variant"], k["props"]) for k in group["kids"]]
        self.assertEqual(got, [("Import", "Button/Secondary", "Size=Sm, State=Default", {"Label": "Import"}),
                               ("New board", "Button/Secondary", "Size=Sm, State=Default", {"Label": "+ FigJam board"}),
                               ("New file", "Button/Primary", "Size=Sm, State=Default", {"Label": "+ Design file"})])

    def test_filter_and_sort_are_secondary_look_frames_with_a_trailing_chevron(self):
        """`paint_secondary(.., chevron = true)` (:1544-1556): the Secondary look, the label at left + `SPACE_GROUP`, `ChevronDown` of `CHEVRON_SMALL` centred at right - `SPACE_GROUP`; width = text +
        3 * `SPACE_GROUP` (:1379, :1391): 128.25 and 153 at x 32 and 168.25, y 4 below the header (home-filter 296, 100; home-sort 432.25, 100)."""
        look = ENV.look("Button/Secondary", "Default")
        for name, text, x, w in (("Filter", "Filter: All files", 32, 128.25), ("Sort", "Sort: Last viewed", 168.25, 153)):
            box = part(self.root, name)
            self.assertEqual((box["abs"], box["fill"], box["stroke"], box["radius"]), ((x, 68, w, 36), look["fill"], look["border"], "radius/control"), name)
            label, chevron = box["kids"]
            self.assertEqual((label["text"], label["style"], label["color"], label["sx"]), (text, "Text sm / Semibold", look["text"], "fill"), name)
            self.assertEqual((chevron["icon"], chevron["size"], chevron["color"]), ("ChevronDown", 10, look["icon"]), name)
            self.assertEqual(pads(box)[3], "space/group", name)

    def test_the_view_toggles_are_the_small_icon_button_with_the_grid_view_selected(self):
        """:1402-1412: two `kit::button_icon` `Size::Sm` squares of `CONTROL_SM` at x = right - pad - 36 and 40 further left: list at 1108 (Default), grid at 1068 (Selected: `h.view == mode`)."""
        for name, icon, state, x in (("List view", "ListView", "Default", 1108), ("Grid view", "GridView", "Selected", 1068)):
            button = part(self.root, name)
            self.assertEqual((button["set"], button["variant"], button["props"], button["abs"]), ("Button/Icon", f"Size=Sm, State={state}", {"Icon": icon}, (x, 68, 36, 36)), name)

    def test_the_header_ends_in_a_full_width_border_hairline_at_y_110(self):
        """:1415 `widgets::hline(&p, rect.left(), rect.right(), top, BORDER)`, `top` = 64 + 4 + 36 + 10 = 114 less the capture's 4: the line at y 110 (142 in the window)."""
        rule = part(self.root, "Rule under header")
        self.assertEqual((rule["set"], rule["abs"]), ("Divider/Hairline", (0, 110, 1176, 1)))

    def test_the_default_content_is_the_file_card_32_in_with_the_home_padding_and_gap(self):
        """`files_view` (:2452-2455): `y = content.top() + pad`, `x0 = content.left() + pad` with `HOME_PAD` 32; the card is `HOME_CARD_W` x 202, columns `HOME_CARD_GAP` apart. The card is at
        (296, 175) in the capture = (32, 143) in the region = 32 below the hairline. Its text is the file's data: the elided meta is what `widgets::elide` shows at the card width."""
        content = part(self.root, "Content")
        self.assertEqual((content["dir"], pads(content), content["gap"]), ("H", ["layout/home-pad"] * 4, "layout/home-card-gap"))
        card = part(self.root, "File card")
        self.assertEqual((card["set"], card["variant"], card["props"]), ("Card/FileThumbnail", "", {"Title": "DESIGN-OS Studio Starter", "Meta": "Edited just now · In Getting st…"}))

    def test_every_text_uses_an_existing_style_and_a_semantic_colour(self):
        names = {s["name"] for s in PLAN["semantic"]}
        for k in walk(self.root):
            if k["t"] == "T":
                self.assertIn(k["style"], STYLES)
                self.assertIn(k["color"], names)


if __name__ == "__main__":
    unittest.main()
