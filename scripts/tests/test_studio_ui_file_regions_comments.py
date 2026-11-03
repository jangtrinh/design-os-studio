"""Panel/Comments of components/regions_sidepanels_comments.py (page `03 Studio`), read from the IR the builder returns against the real Semantic plan. Lines are the v0.7.5 tag
(`comments/panel.rs`, `comments/ui.rs`); the capture is `j-comments`, the region is the right panel (1199, 32, 241, 868), so every rect here is relative to that origin.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_comments
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio_ui_regions_fixtures as fx  # noqa: E402
from rest_ir import walk  # noqa: E402


class CommentsRegionTest(unittest.TestCase):
    def setUp(self):
        self.region = fx.build("Panel/Comments")

    def test_it_is_one_plain_record_nesting_tabs_search_close_and_hairline_sets(self):
        spec = fx.SPECS["Panel/Comments"]
        self.assertEqual(spec["axes"], {})
        self.assertEqual(sorted(spec["uses"]), ["Button/Close", "Divider/Hairline", "Input/Field", "Tabs/Segmented"])
        self.assertEqual(spec["props"], [{"name": "Threads", "type": "SLOT"}])

    def test_the_frame_is_the_right_panel_in_bg_with_a_one_px_left_edge(self):
        r = self.region
        self.assertEqual((r["w"], r["h"], r["fill"], r.get("clip")), (241, 868, "color/bg", True))
        self.assertEqual((fx.part(r, "Left edge")["abs"], fx.part(r, "Left edge")["fill"]), ((0, 0, 1, 868), "color/border"))

    def test_the_header_is_a_large_semibold_title_16_in_and_a_close_button_8_from_the_right(self):
        title = fx.part(self.region, "Title")
        self.assertEqual((title["abs"], fx.texts(title), fx.part(title, "Text")["style"]), ((0, 0, 241, 48), ["Comments"], "Large / Semibold"))
        close = fx.part(self.region, "Close")
        self.assertEqual((close["set"], close["abs"]), ("Button/Close", (209, 12, 24, 24)), "comments/panel.rs:34: right - 8 - 24, COMMENT_PANEL_HEADER_H / 2 - 12")

    def test_the_two_filters_are_selected_segmented_strips_216_wide_8_apart(self):
        pages, status = fx.part(self.region, "Page filter"), fx.part(self.region, "Status filter")
        self.assertEqual((pages["set"], pages["variant"], pages["abs"]), ("Tabs/Segmented", "State=Selected", (13, 57, 216, 24)))
        self.assertEqual(pages["props"], {"First label": "This page", "Second label": "All pages"})
        self.assertEqual((status["abs"], status["props"]), ((13, 89, 216, 24), {"First label": "Open", "Second label": "Resolved"}))

    def test_the_search_is_the_empty_xs_input_field_with_the_callers_own_glyph_and_hint(self):
        field = fx.part(self.region, "Search field")
        self.assertEqual((field["set"], field["variant"], field["abs"], field["props"]), ("Input/Field", "Size=Xs, State=Default", (13, 121, 216, 32), {"Has Value": False}))
        icon = fx.part(self.region, "Search icon")
        glyphs = [i for i in walk(icon) if i["t"] == "I"]
        self.assertEqual(([i["icon"] for i in glyphs], [i["size"] for i in glyphs], icon["abs"]), (["Search"], [16], (19, 129, 16, 16)), "comments/panel.rs:64: centred at left + 14")
        hint = fx.part(self.region, "Search hint")
        self.assertEqual((hint["abs"], fx.texts(hint)), ((41, 121, 180, 32), ["Search comments"]), "comments/panel.rs:75: from left + 28 to right - 8")
        self.assertEqual(fx.part(hint, "Text")["color"], "color/text-placeholder")

    def test_two_hairlines_close_the_header_and_the_search_row_and_a_third_opens_the_footer(self):
        rules = [k for k in self.region["kids"] if k["t"] == "X" and k["set"] == "Divider/Hairline"]
        self.assertEqual([k["abs"] for k in rules], [(1, 48, 240, 1), (1, 161, 240, 1), (1, 828, 240, 1)], "comments/panel.rs:40, :88 and :93: hline(rect.left() + 1, rect.right(), y)")

    def test_the_threads_are_a_clipped_slot_from_y_162_holding_the_capture_card(self):
        slot = fx.part(self.region, "Threads")
        self.assertEqual((slot["abs"], slot["slot"], slot["dir"], slot.get("clip")), ((1, 162, 240, 666), "Threads", "V", True))
        self.assertEqual([fx.lit_of(p) for p in slot["pad"]], [("lit", 8.0), ("lit", 12.0), None, ("lit", 12.0)])
        self.assertEqual([k["name"] for k in slot["kids"]], ["Thread 1"])

    def test_an_open_thread_card_is_the_bg_selected_pill_with_a_border_65_px_high(self):
        card = fx.part(self.region, "Thread 1")
        self.assertEqual((card["w"], card["h"], card["fill"], card["stroke"], card["sw"], card["radius"]), (216, 65, "color/bg-selected", "color/border", "size/border-width", "radius/control"))
        avatar = fx.part(card, "Avatar")
        self.assertEqual((avatar["abs"], avatar["fill"], fx.texts(avatar)), ((12, 12, 20, 20), "color/avatar-colors-0", ["A"]), "comments/panel.rs:170-172: 20 px circle at the card's padding")
        self.assertEqual(fx.lit_of(avatar["radius"]), ("lit", 10.0))

    def test_a_thread_card_has_the_author_the_age_and_the_first_message(self):
        card = fx.part(self.region, "Thread 1")
        name, age, message = fx.part(card, "Author"), fx.part(card, "Age"), fx.part(card, "Message")
        self.assertEqual([(t["text"], t["style"], t["color"]) for t in (name, age, message)],
                         [("Agent", "Body / Semibold", "color/text"), ("just now", "Caption / Regular", "color/text-tertiary"), ("Align icon spacing to 8px grid", "Body / Regular", "color/text")])

    def test_the_footer_is_an_unchecked_14_px_box_and_the_show_resolved_label(self):
        box = fx.part(self.region, "Show resolved box")
        self.assertEqual((box["abs"], box["stroke"], box["radius"], box.get("fill")), ((13, 841, 14, 14), "color/text-tertiary", "radius/mark", None))
        self.assertEqual(fx.lit_of(box["sw"]), ("lit", 1.0))
        text = fx.part(self.region, "Show resolved")
        self.assertEqual((text["abs"], fx.texts(text)), ((35, 836, 120, 24), ["Show resolved"]))

    def test_no_kit_name_is_in_the_region_and_every_text_style_and_literal_is_defined(self):
        for t in fx.texts(self.region):
            self.assertFalse(any(w in t.lower() for w in fx.KIT_WORDS), t)
        for k in walk(self.region):
            if k["t"] == "T":
                self.assertIn(k["style"], fx.STYLES)
        found = fx.lit_reasons(self.region)
        self.assertTrue(found)
        for name, e in found:
            self.assertRegex(e[2], r"\w+\.rs:\d+", f"{name}: lit({e[1]})")


if __name__ == "__main__":
    unittest.main()
