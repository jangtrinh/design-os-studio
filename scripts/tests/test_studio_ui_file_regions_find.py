"""Panel/FindReplace of components/regions_sidepanels_find.py (page `03 Studio`), read from the IR the builder returns against the real Semantic plan. Lines are the v0.7.5 tag
(`find/panel.rs`); the capture is `j-find`, the region is the left panel below the pages list: (49, 305, 240, 595), so every rect here is that origin.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_find
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio_ui_regions_fixtures as fx  # noqa: E402
from rest_ir import walk  # noqa: E402


class FindRegionTest(unittest.TestCase):
    def setUp(self):
        self.region = fx.build("Panel/FindReplace")

    def test_it_is_one_plain_record_nesting_the_close_icon_field_and_hairline_sets(self):
        spec = fx.SPECS["Panel/FindReplace"]
        self.assertEqual(spec["axes"], {})
        self.assertEqual(sorted(spec["uses"]), ["Button/Close", "Button/Icon", "Divider/Hairline", "Input/Field"])
        self.assertEqual(spec["props"], [{"name": "Results", "type": "SLOT"}])

    def test_the_frame_is_the_left_panel_below_the_pages_list_in_bg(self):
        r = self.region
        self.assertEqual((r["w"], r["h"], r["fill"], r["sx"], r["sy"], r.get("clip")), (240, 595, "color/bg", "fixed", "fixed", True))

    def test_the_title_is_body_semibold_at_the_panel_padding_and_the_two_buttons_sit_8_from_the_right(self):
        title = fx.part(self.region, "Title")
        self.assertEqual((title["abs"], fx.texts(title), title["pad"][3]), ((0, 0, 240, 40), ["Find and replace"], "layout/panel-padding-x"))
        self.assertEqual(fx.part(title, "Text")["style"], "Body / Semibold")
        close, toggle = fx.part(self.region, "Close"), fx.part(self.region, "Replace toggle")
        self.assertEqual((close["set"], close["abs"]), ("Button/Close", (208, 8, 24, 24)), "find/panel.rs:191-192: x1 - PANEL_ACTION_RIGHT_PAD - PANEL_ACTION_BUTTON, y + 8")
        self.assertEqual((toggle["set"], toggle["props"], toggle["abs"]), ("Button/Icon", {"Icon": "ChevronRight"}, (180, 8, 24, 24)), "find/panel.rs:203-205: SPACE_FACT left of the close button")

    def test_the_search_field_is_the_xs_input_field_138_wide_and_the_three_toggles_follow_it(self):
        field = fx.part(self.region, "Search field")
        self.assertEqual((field["set"], field["variant"], field["abs"], field["props"]), ("Input/Field", "Size=Xs, State=Default", (16, 40, 138, 32), {"Value": "Button"}))
        toggles = [fx.part(self.region, n) for n in ("Match case", "Whole word", "Regex")]
        self.assertEqual([t["abs"] for t in toggles], [(156, 45, 22, 22), (178, 45, 22, 22), (200, 45, 22, 22)], "find/panel.rs:221-227: the right 70 px of the 208 px row, 22 px each")
        self.assertEqual([fx.texts(t) for t in toggles], [["Aa"], ["ab"], [".*"]])
        self.assertEqual({fx.part(t, "Text")["color"] for t in toggles}, {"color/icon-secondary"})
        self.assertEqual({fx.part(t, "Text")["style"] for t in toggles}, {"Body / Semibold"})

    def test_the_status_row_has_the_count_the_scope_chip_and_the_two_step_buttons(self):
        status = fx.part(self.region, "Count")
        self.assertEqual((status["abs"], fx.texts(status)), ((16, 80, 92, 32), ["19 results"]))
        self.assertEqual((fx.part(status, "Text")["style"], fx.part(status, "Text")["color"]), ("Caption / Regular", "color/text-secondary"))
        scope = fx.part(self.region, "Scope")
        self.assertEqual((scope["abs"], fx.texts(scope), scope["sx"]), ((112, 85, 68, 22), ["This page"], "hug"),
                         "find/panel.rs:281-282: text_width + 12 wide, 22 high, 4 left of the previous button")
        self.assertEqual([fx.part(self.region, n)["abs"] for n in ("Previous", "Next")], [(184, 84, 24, 24), (208, 84, 24, 24)])
        glyphs = [i["icon"] for n in ("Previous", "Next") for i in walk(fx.part(self.region, n)) if i["t"] == "I"]
        self.assertEqual(glyphs, [], "the chevrons are strokes (test_studio_ui_file_stroked_glyphs), not glyph instances")

    def test_one_divider_hairline_closes_the_status_row_on_its_last_pixel(self):
        rules = [k for k in self.region["kids"] if k["t"] == "X" and k["set"] == "Divider/Hairline"]
        self.assertEqual([k["abs"] for k in rules], [(0, 111, 240, 1)], "find/panel.rs:299: hline(x0, x1, y - 1) with y = 80 + FIND_STATUS_H")

    def test_the_results_are_a_clipped_slot_of_32_px_group_and_hit_rows_from_y_112(self):
        slot = fx.part(self.region, "Results")
        self.assertEqual((slot["abs"], slot["slot"], slot["dir"], slot.get("clip")), ((0, 112, 240, 483), "Results", "V", True))
        self.assertEqual({k["h"] for k in slot["kids"]}, {32.0}, "FIND_GROUP_ROW_H and LAYER_ROW_HEIGHT are both 32")
        self.assertEqual([k["name"].split()[0] for k in slot["kids"]][:4], ["Group", "Hit", "Group", "Hit"])
        self.assertEqual(len(slot["kids"]), 15, "7 frames with a hit each and the next frame's row, as far as the 483 px list shows")

    def test_a_group_row_is_a_tertiary_frame_glyph_at_16_and_a_secondary_name_8_after_it(self):
        row = fx.part(self.region, "Group 1")
        self.assertEqual([k["t"] for k in row["kids"]], ["I", "T"])
        glyph, name = row["kids"]
        self.assertEqual((glyph["icon"], glyph["size"], glyph["color"]), ("AutoLayoutH", 16, "color/icon-tertiary"))
        self.assertEqual((name["style"], name["color"], row["gap"], row["pad"][3]), ("Body / Regular", "color/text-secondary", "space/inline", "inset/panel"))

    def test_a_hit_row_indents_16_more_and_marks_the_match_under_the_text(self):
        row = fx.part(self.region, "Hit 1")
        mark, glyph, text = row["kids"]
        self.assertEqual((mark["name"], mark["fill"], mark["radius"]), ("Match", "color/find-highlight", "radius/mark"))
        self.assertEqual(mark["abs"], (111.08, 7, 36.47, 18), "find/panel.rs:416-419: rr.center.y +- 9, from the match's start to its end in `tx` + text_width")
        self.assertEqual((glyph["icon"], text["text"], text["color"], row["pad"][3][1]), ("AutoLayoutH", "Card and button", "color/text", 32.0))

    def test_the_scrollbar_thumb_is_the_capture_196_px_tall_4_wide_6_from_the_right(self):
        thumb = fx.part(self.region, "Scrollbar thumb")
        self.assertEqual((thumb["abs"], thumb["fill"]), ((234, 112, 4, 196), "color/text-tertiary"))

    def test_no_kit_or_document_name_is_in_the_region_and_every_text_style_is_defined(self):
        for t in texts_of(self.region):
            self.assertFalse(any(w in t.lower() for w in fx.KIT_WORDS), t)
        for k in walk(self.region):
            if k["t"] == "T":
                self.assertIn(k["style"], fx.STYLES)

    def test_every_pinned_literal_names_its_rust_line(self):
        found = fx.lit_reasons(self.region)
        self.assertTrue(found)
        for name, e in found:
            self.assertRegex(e[2], r"\w+\.rs:\d+", f"{name}: lit({e[1]})")


def texts_of(node):
    return fx.texts(node)


if __name__ == "__main__":
    unittest.main()
