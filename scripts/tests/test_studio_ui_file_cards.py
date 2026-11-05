"""The Cards row of page `03 Studio` (components/cards.py): Card/Library and Card/FileThumbnail, read from the IR the builders return against the real Semantic plan, with
the Rust each value comes from named in cards.py. Structure only: pixels are compared in `evidence compare`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_cards
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
from components import cards  # noqa: E402
from components.env import Env  # noqa: E402
from rest_ir import walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPECS = {s["name"]: s for s in components.discover() if s["module"] in ("cards", "cards_templates")}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def build(name, **values):
    return SPECS[name]["build"](values, ENV)


def part(node, name):
    found = [k for k in walk(node) if k["name"] == name]
    assert len(found) == 1, (name, len(found))
    return found[0]


class LibraryCardTest(unittest.TestCase):
    """`library_card` (panels/assets.rs:397-415)."""

    def setUp(self):
        self.spec, self.card = SPECS["Card/Library"], build("Card/Library")

    def test_it_is_a_plain_record_on_the_cards_row(self):
        self.assertEqual((self.spec["axes"], self.spec["row"]), ({}, "Cards"))

    def test_the_card_is_the_callers_rect_208_by_86_with_no_paint_of_its_own(self):
        """`region.width - 2 * INSET_PANEL` = 240 - 32 = 208 wide (assets.rs:550-553), 86 high; the card rect itself is never filled or stroked."""
        self.assertEqual((self.card["w"], self.card["h"]), (208, 86))
        self.assertEqual({k for k in ("fill", "stroke", "radius") if self.card.get(k)}, set())

    def test_the_preview_frame_is_52_high_bg_secondary_with_a_border_at_radius_control(self):
        frame = part(self.card, "Preview")
        self.assertEqual(frame["abs"], (0, 0, 208, 52))
        self.assertEqual((frame["fill"], frame["stroke"], frame["sw"], frame["radius"]), ("color/bg-secondary", "color/border", "size/border-width", "radius/control"))

    def test_the_preview_glyph_is_the_18_px_component_icon_in_icon_tertiary_centred(self):
        frame = part(self.card, "Preview")
        glyph = [k for k in frame["kids"] if k["t"] == "I"]
        self.assertEqual([(g["icon"], g["size"], g["color"]) for g in glyph], [("Component", 18, "color/icon-tertiary")])
        self.assertEqual((frame["dir"], frame["align"]), ("H", ("center", "center")))

    def test_the_two_lines_overlap_by_three_px_so_they_are_placed_not_stacked(self):
        """Their centres are 11 and 26 below the preview, 15 apart, while a line is 18 high (assets.rs:405-414)."""
        title, count = part(self.card, "Title"), part(self.card, "Count")
        self.assertEqual((title["abs"][:2], count["abs"][:2]), ((2, 54), (2, 69)))
        self.assertLess(count["abs"][1], title["abs"][1] + title["abs"][3])

    def test_title_and_count_follow_the_painters_fonts_and_colours(self):
        title, count = part(self.card, "Title"), part(self.card, "Count")
        self.assertEqual((title["style"], title["color"], title["prop"]), ("Body / Semibold", "color/text", "Title"))
        self.assertEqual((count["style"], count["color"], count["prop"]), ("Caption / Regular", "color/text-tertiary", "Count"))

    def test_every_style_the_card_uses_exists(self):
        self.assertLessEqual({k["style"] for k in walk(self.card) if k["t"] == "T"}, STYLES)

    def test_the_text_properties_are_declared(self):
        self.assertEqual([(p["name"], p["type"]) for p in self.spec["props"]], [("Title", "TEXT"), ("Count", "TEXT")])


class FileThumbnailCardTest(unittest.TestCase):
    """`card` (home/mod.rs:2775-2817) with `thumb_area` (:2702-2735) and `draw_file_type_badge` (:2755-2766)."""

    def setUp(self):
        self.spec, self.card = SPECS["Card/FileThumbnail"], build("Card/FileThumbnail")

    def test_it_is_a_plain_record_on_the_cards_row_nesting_the_tile_and_the_hairline(self):
        self.assertEqual((self.spec["axes"], self.spec["row"]), ({}, "Cards"))
        self.assertEqual(sorted(self.spec["uses"]), ["Divider/Hairline", "Icon/Tile"])

    def test_the_card_is_the_home_grid_card_224_by_202_filled_bg_secondary_with_a_border_at_radius_xl(self):
        """HOME_CARD_W by HOME_THUMB_H + CARD_META_H (12 + 20 + 18 + 12 = 62), `rect_filled(r, RADIUS_XL, BG_SECONDARY)` then a 1 px inside `BORDER` stroke (:2777, :2784-2791)."""
        self.assertEqual((self.card["w"], self.card["h"]), (224, 202))
        self.assertEqual((self.card["fill"], self.card["stroke"], self.card["sw"][:2], self.card["radius"]), ("color/bg-secondary", "color/border", ("lit", 1.0), "radius/xl"))
        self.assertIn("home/mod.rs:2789", self.card["sw"][2])  # `Stroke::new(1.0, theme::BORDER)`: a raw 1.0, BORDER_WIDTH is not named there (the Library card's `kit::look` does name it)

    def test_the_thumbnail_is_140_high_rounds_all_four_corners_and_holds_the_24_px_file_glyph(self):
        thumb = part(self.card, "Thumbnail")
        self.assertEqual((thumb["abs"], thumb["fill"], thumb["radius"]), ((0, 0, 224, 140), "color/home-thumb-bg", "radius/xl"))
        self.assertEqual([(k["icon"], k["size"], k["color"]) for k in thumb["kids"] if k["t"] == "I"], [("File", 24, "color/icon-tertiary")])

    def test_the_divider_is_the_hairline_set_along_the_thumbnails_bottom_edge_inset_one_px(self):
        """`widgets::hline(p, thumb.left() + 1.0, thumb.right() - 1.0, thumb.bottom(), BORDER)` (:2783); `hline` is Divider/Hairline's painter."""
        line = part(self.card, "Divider")
        self.assertEqual((line["t"], line["set"], line["variant"], line["abs"]), ("X", "Divider/Hairline", "State=Default", (1, 140, 222, 1)))

    def test_the_type_badge_is_the_design_tile_at_sm_centred_on_18_162(self):
        tile = part(self.card, "File type")
        self.assertEqual((tile["t"], tile["set"], tile["variant"], tile["abs"]), ("X", "Icon/Tile", "Kind=Design, Size=Sm", (8, 152, 20, 20)))

    def test_title_and_meta_start_at_34_and_sit_on_the_painters_line_boxes(self):
        """Title centred at thumb.bottom + SPACE_GROUP + LINE_TEXT_SM / 2 = 162 in a 20 px line; meta 10 + 9 below that, in an 18 px line (:2796-2810)."""
        title, meta = part(self.card, "Title"), part(self.card, "Meta")
        self.assertEqual((title["abs"][:2], meta["abs"][:2]), ((34, 152), (34, 172)))
        self.assertEqual((title["style"], title["color"], title["prop"]), ("Text sm / Semibold", "color/text", "Title"))
        self.assertEqual((meta["color"], meta["prop"]), ("color/text-tertiary", "Meta"))

    def test_the_meta_style_is_text_xs_regular_the_style_the_painter_names(self):
        """`font_text(FONT_SIZE_TEXT_XS, Regular)` is the `Text xs / Regular` style, which text_styles_plan.ROLES holds; the module keeps no stand-in constant."""
        self.assertEqual(part(self.card, "Meta")["style"], "Text xs / Regular")
        self.assertFalse(hasattr(cards, "XS_REGULAR"))

    def test_every_style_the_card_uses_exists(self):
        self.assertLessEqual({k["style"] for k in walk(self.card) if k["t"] == "T"}, STYLES)

    def test_the_text_properties_are_declared_and_the_defaults_are_neutral(self):
        self.assertEqual([(p["name"], p["type"]) for p in self.spec["props"]], [("Title", "TEXT"), ("Meta", "TEXT")])
        words = " ".join(k["text"] for k in walk(self.card) if k["t"] == "T")
        self.assertNotIn("untitled", words.lower())


if __name__ == "__main__":
    unittest.main()
