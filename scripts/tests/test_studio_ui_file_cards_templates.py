"""Card/StarterTemplate (row Cards) and EmptyState/Default (row Bars & Feedback) of components/cards_templates.py, read from the IR the builders return against the real
Semantic plan, with the Rust each value comes from named in the module. Structure only: pixels are compared in `evidence compare` (neither set has a capture).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_cards_templates
"""
import re
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
from components import cards_templates as tpl  # noqa: E402
from components.env import Env  # noqa: E402
from rest_ir import pads, walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPECS_ALL = {s["name"]: s for s in components.discover()}
SPECS = {s["name"]: s for s in SPECS_ALL.values() if s["module"] == "cards_templates"}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def build(name, **values):
    return SPECS[name]["build"](values, ENV)


def part(node, name):
    found = [k for k in walk(node) if k["name"] == name]
    assert len(found) == 1, (name, len(found))
    return found[0]


class StarterTemplateTest(unittest.TestCase):
    """`templates_view` template cards (home/mod.rs:1634-1815), the first card (`Blank design`)."""

    def setUp(self):
        self.spec, self.card = SPECS["Card/StarterTemplate"], build("Card/StarterTemplate")

    def test_it_is_a_plain_record_on_the_cards_row_and_nests_the_secondary_button(self):
        """Button/Secondary Sm is the button's painter (`paint_secondary` is `button_look(Secondary, Default)` + `paint_frame` at RADIUS_CONTROL and a centred `Text sm` Semibold label), and a Fill copy
        of its hug main is not re-hugged now that the copy writes its own sizing (the generator used to keep the main's Hug, so it stayed 60.75 px wide)."""
        self.assertEqual((self.spec["axes"], self.spec["row"], self.spec.get("uses", [])), ({}, "Cards", ["Button/Secondary"]))
        self.assertEqual([(k["set"], k["variant"]) for k in walk(self.card) if k["t"] == "X"], [("Button/Secondary", "Size=Sm, State=Default")])

    def test_the_card_is_240_by_204_xl_radius_with_a_border_and_clips_its_banner(self):
        """`tcw` 240, `tch` = 188 + SPACE_GROUP + SPACE_FACT (:1688); the banner's top corners are the card's own (a clip), as only they are rounded in the Rust."""
        self.assertEqual((self.card["w"], self.card["h"]), (240, 204))
        self.assertEqual((self.card["fill"], self.card["stroke"], self.card["sw"][:2], self.card["radius"], self.card["clip"]),
                         ("color/bg-secondary", "color/border", ("lit", 1.0), "radius/xl", True))
        self.assertIn("home/mod.rs:1730", self.card["sw"][2])  # `Stroke::new(if hot { 1.5 } else { 1.0 }, ..)`: raw widths; the icon box and the pill name `theme::BORDER_WIDTH`

    def test_the_button_is_the_nested_secondary_sm_filling_the_card_between_its_padding_at_the_bottom(self):
        """`btn_r` is `r.bottom() - SPACE_GROUP - CONTROL_SM` and as wide as the card less 2 * SPACE_GROUP (:1722-1725): a bottom-aligned child with that padding, filling the width; its main is
        36 high (CONTROL_SM), so only the width is the copy's own. The label is the nested button's own `Label` property, `New file`."""
        card, button = self.card, part(self.card, "Action")
        self.assertEqual((card["dir"], card["align"][0]), ("V", "end"))
        self.assertEqual(pads(card), [None, "space/group", "space/group", "space/group"])
        self.assertEqual((button["t"], button["set"], button["variant"], button["sx"], button.get("sy"), button.get("abs")), ("X", "Button/Secondary", "Size=Sm, State=Default", "fill", None, None))
        self.assertEqual(button["props"], {"Label": "New file"})
        self.assertNotIn("Action", [p["name"] for p in self.spec["props"]], "the card has no property to drive a nested copy's label")

    def test_the_card_nests_the_button_main_and_the_main_is_36_high_and_hugs_its_width(self):
        main = SPECS_ALL["Button/Secondary"]["build"]({"Size": "Sm", "State": "Default"}, ENV)
        self.assertEqual((main["h"], main["sx"], main["sy"]), (ENV.num("CONTROL_SM"), "hug", "fixed"))
        self.assertEqual((main["fill"], main["radius"]), (ENV.look("Button/Secondary", "Default")["fill"], "radius/control"))

    def test_the_banner_is_72_high_inside_the_stroke_in_the_design_tile_fill(self):
        banner = part(self.card, "Banner")
        self.assertEqual((banner["abs"], banner["fill"], [c[:2] if c else None for c in banner["radius"]]), ((1, 1, 238, 72), "color/tile-design-bg", [("lit", 11.0), ("lit", 11.0), None, None]))

    def test_the_icon_box_is_28_square_at_12_14_with_the_tile_fill_a_border_and_a_15_px_glyph(self):
        box = part(self.card, "Icon box")
        self.assertEqual((box["abs"], box["fill"], box["stroke"], box["radius"]), ((12, 14, 28, 28), "color/tile-design-bg", "color/border", "radius/control"))
        self.assertEqual([(k["icon"], k["size"], k["color"], k["swap"]) for k in box["kids"] if k["t"] == "I"], [("Pen", 15, "color/tile-design-glyph", "Icon")])

    def test_the_badge_is_a_20_px_pill_of_radius_10_padded_7_with_the_tile_fill_and_a_border(self):
        pill = part(self.card, "Badge")
        self.assertEqual((pill["h"], pill["fill"], pill["stroke"], pill["radius"][1:2], pads(pill)[1][1:2]), (20, "color/tile-design-bg", "color/border", (10.0,), (7.0,)))
        self.assertEqual(pill["abs"][1], 16)
        label = part(pill, "Label")
        self.assertEqual((label["style"], label["color"], label["prop"]), ("Text xs / Semibold", "color/tile-design-glyph", "Badge"))

    def test_the_two_bars_sit_14_above_the_banners_bottom_at_radius_mark(self):
        for name, x, w in (("Bar short", 12, 64), ("Bar long", 82, 40)):
            bar = part(self.card, name)
            self.assertEqual((bar["abs"], bar["fill"], bar["radius"]), ((x, 59, w, 4), "color/tile-design-glyph", "radius/mark"), name)

    def test_title_and_description_follow_the_painters_fonts_and_lines(self):
        title, desc = part(self.card, "Title"), part(self.card, "Description")
        self.assertEqual((title["abs"][:2], title["style"], title["color"]), ((12, 79), "Text sm / Semibold", "color/text"))
        self.assertEqual((desc["abs"][:2], desc["style"], desc["color"], desc["wrap"], desc["w"]), ((12, 101), "Text xs / Regular", "color/text-tertiary", True, 216))

    def test_no_caption_stand_in_constant_is_left(self):
        """`Text xs` Regular and Semibold are ROLES lines now: the description and the badge name them, and the module keeps no stand-in constant."""
        self.assertFalse(hasattr(tpl, "XS_REGULAR") or hasattr(tpl, "XS_SEMIBOLD"))

    def test_every_style_exists_and_the_props_are_declared(self):
        self.assertLessEqual({k["style"] for k in walk(self.card) if k["t"] == "T"}, STYLES)
        self.assertEqual([(p["name"], p["type"]) for p in self.spec["props"]],
                         [("Title", "TEXT"), ("Badge", "TEXT"), ("Description", "TEXT"), ("Icon", "INSTANCE_SWAP")])


class EmptyStateTest(unittest.TestCase):
    """The Home `empty_state` (home/mod.rs:2450-2474): a tile 44 above the centre, a title at it, a body 4 below the title's line."""

    def setUp(self):
        self.spec, self.state = SPECS["EmptyState/Default"], build("EmptyState/Default", State="Default")

    def test_it_is_the_default_state_on_the_bars_and_feedback_row_nesting_the_neutral_tile(self):
        self.assertEqual((self.spec["axes"], self.spec["row"], self.spec["uses"]), ({"State": ["Default"]}, "Bars & Feedback", ["Icon/Tile"]))
        tile = part(self.state, "Tile")
        self.assertEqual((tile["set"], tile["variant"], tile.get("abs")), ("Icon/Tile", "Kind=Neutral, Size=Md", None))

    def test_the_column_is_centred_and_hugs_its_content(self):
        self.assertEqual((self.state["dir"], self.state["align"], self.state.get("sx", "hug"), self.state.get("sy", "hug")), ("V", ("start", "center"), "hug", "hug"))

    def test_the_body_sits_space_fact_below_the_title_and_the_title_16_below_the_tile(self):
        """Tile bottom c - 28, title line c - 12 .. c + 12, body top c + 12 + SPACE_FACT: a heading of tile and title, then the body."""
        heading = part(self.state, "Heading")
        self.assertEqual([k["name"] for k in heading["kids"]], ["Tile", "Title"])
        self.assertEqual(heading["gap"][1:2], (16.0,))
        self.assertEqual(self.state["gap"], "space/fact")
        self.assertEqual([k["name"] for k in self.state["kids"]], ["Heading", "Body"])

    def test_title_and_body_follow_the_painters_fonts_and_colours(self):
        title, body = part(self.state, "Title"), part(self.state, "Body")
        self.assertEqual((title["style"], title["color"], title["prop"]), ("Text md / Semibold", "color/text", "Title"))
        self.assertEqual((body["style"], body["color"], body["prop"], body.get("wrap")), ("Text sm / Regular", "color/text-tertiary", "Body", True))

    def test_the_body_is_the_painters_default_wrapped_at_380_with_left_aligned_lines_in_a_block_the_column_centres(self):
        """`empty_state` lays the body out at `min(380, width - 48)` with egui's default alignment (`p.layout`, no `halign`), then places the galley at `c.x - galley.size().x / 2`: the BLOCK is
        centred and its lines are left-aligned. Measured on the 0.7.5 capture of the empty Home (h-home-empty): line 1 starts at x 667, line 2 at x 668. So the body carries no `align_h`: centring
        each line (`align_h="center"`) would move line 2 about 85 px away from where production draws it."""
        body = part(self.state, "Body")
        self.assertEqual(body["text"], "Create a design file, FigJam board, or import a .fig, .svg or .designos file to get started.")
        self.assertEqual((body["w"], body.get("sx")), (380, "fixed"))
        self.assertNotIn("align_h", body)
        self.assertEqual(self.state["align"], ("start", "center"), "the column centres the 380 px block under the title")

    def test_the_wrap_width_citations_point_at_the_line_that_holds_the_380(self):
        """`BODY_W` and the module docstring cite the line of `380.0_f32.min(area.width() - 48.0)` in home/mod.rs: that is 2470-2471 (2469 is the title's `p.text`)."""
        source = (HERE.parent / "studio_ui_file" / "components" / "cards_templates.py").read_text(encoding="utf-8")
        rust = (REPO / "crates" / "designos-app" / "src" / "home" / "mod.rs").read_text(encoding="utf-8").splitlines()
        cited = re.findall(r"(?:home/mod\.rs|:)(\d+)-(\d+)", "\n".join(l for l in source.splitlines() if "380" in l and ("area.width()" in l or "min(380" in l)))
        self.assertEqual(len(cited), 2, "the docstring and BODY_W each cite the wrap width's lines")
        for first, last in cited:
            self.assertEqual((first, last), ("2470", "2471"))
            self.assertIn("380.0_f32.min(area.width() - 48.0)", " ".join(rust[int(first) - 1:int(last)]))
        self.assertNotIn("2469", "\n".join(l for l in source.splitlines() if "380" in l and "area.width()" in l))

    def test_every_style_exists_and_the_props_are_declared(self):
        self.assertLessEqual({k["style"] for k in walk(self.state) if k["t"] == "T"}, STYLES)
        self.assertEqual([(p["name"], p["type"]) for p in self.spec["props"]], [("Title", "TEXT"), ("Body", "TEXT")])


if __name__ == "__main__":
    unittest.main()
