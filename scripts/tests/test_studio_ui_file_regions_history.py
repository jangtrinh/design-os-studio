"""Panel/VersionHistory of components/regions_sidepanels_history.py (page `03 Studio`), read from the IR the builder returns against the real Semantic plan. Lines are the v0.7.5 tag
(`history/panel.rs`); the capture is `j-version-history`, the region is the right panel (1199, 32, 241, 868) in the state of that capture: a saved version is being viewed.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_history
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio_ui_regions_fixtures as fx  # noqa: E402
from rest_ir import walk  # noqa: E402


class HistoryRegionTest(unittest.TestCase):
    def setUp(self):
        self.region = fx.build("Panel/VersionHistory")
        self.rows = fx.part(self.region, "Versions")["kids"]

    def test_it_is_one_plain_record_nesting_the_close_secondary_button_and_hairline_sets(self):
        spec = fx.SPECS["Panel/VersionHistory"]
        self.assertEqual(spec["axes"], {})
        self.assertEqual(sorted(spec["uses"]), ["Button/Close", "Button/Secondary", "Divider/Hairline"])
        self.assertEqual(spec["props"], [{"name": "Versions", "type": "SLOT"}])

    def test_the_frame_is_the_right_panel_in_bg_with_a_one_px_left_edge(self):
        r = self.region
        self.assertEqual((r["w"], r["h"], r["fill"], r.get("clip")), (241, 868, "color/bg", True))
        edge = fx.part(r, "Left edge")
        self.assertEqual((edge["abs"], edge["fill"]), ((0, 0, 1, 868), "color/border"), "history/panel.rs:24 `vline(rect.left(), top, bottom, BORDER)`")

    def test_the_title_is_large_semibold_16_in_and_the_close_button_sits_8_from_the_right(self):
        title = fx.part(self.region, "Title")
        self.assertEqual((title["abs"], fx.texts(title), fx.part(title, "Text")["style"]), ((0, 0, 241, 48), ["Version history"], "Large / Semibold"))
        self.assertEqual(title["pad"][3][:2], ("lit", 16.0), "history/panel.rs:27: `rect.left() + 16.0`, a raw literal")
        close = fx.part(self.region, "Close")
        self.assertEqual((close["set"], close["abs"]), ("Button/Close", (209, 12, 24, 24)), "history/panel.rs:33: right - 8 - 24, COMMENT_PANEL_HEADER_H / 2 - 12")

    def test_two_hairlines_start_one_pixel_in_at_the_header_and_below_the_save_button(self):
        rules = [k for k in self.region["kids"] if k["t"] == "X" and k["set"] == "Divider/Hairline"]
        self.assertEqual([k["abs"] for k in rules], [(1, 48, 240, 1), (1, 97, 240, 1)], "history/panel.rs:39 and :61: hline(rect.left() + 1, rect.right(), y)")

    def test_the_save_button_is_the_disabled_secondary_xs_button_across_the_panel_while_a_version_is_viewed(self):
        save = fx.part(self.region, "Save")
        self.assertEqual((save["t"], save["abs"]), ("X", (9, 57, 224, 32)))
        self.assertEqual((save["set"], save["variant"], save["w"]), ("Button/Secondary", "Size=Xs, State=Disabled", 224), "the hug main is resized with `w=`: the copy's own sizing is fixed")
        self.assertEqual(save["props"], {"Label": "Save to version history…"})

    def test_the_list_is_a_clipped_slot_from_y_98_with_44_and_28_px_rows_inset_8(self):
        slot = fx.part(self.region, "Versions")
        self.assertEqual((slot["abs"], slot["slot"], slot["dir"], slot.get("clip"), slot["w"]), ((1, 98, 240, 770), "Versions", "V", True, 240))
        self.assertEqual([fx.lit_of(p) for p in slot["pad"]], [("lit", 4.0), ("lit", 8.0), None, ("lit", 8.0)])
        self.assertEqual([k["h"] for k in self.rows], [44, 28, 44, 44, 44, 44, 44, 28, 44, 44, 28, 44, 28, 44])
        self.assertEqual({k["w"] for k in self.rows}, {224})

    def test_the_current_row_is_unselected_and_the_viewed_version_is_the_selected_pill(self):
        current, last = self.rows[0], self.rows[-1]
        self.assertEqual((current["name"], current.get("fill")), ("Current version", None))
        self.assertEqual((last["name"], last["fill"], last["radius"]), ("Version 9", "color/bg-selected", "radius/control"))
        self.assertEqual(fx.texts(current), ["Current version"])

    def test_a_version_row_has_a_dot_at_16_a_semibold_line_and_a_caption_line(self):
        row = fx.part(self.region, "Version 1")
        dot, first, second = row["kids"]
        self.assertEqual((dot["t"], dot["abs"], dot["fill"]), ("E", (12, 18, 8, 8), "color/icon-tertiary"), "history/panel.rs:147: circle_filled(r.left() + 16, centre, 4)")
        self.assertEqual((first["abs"], first["style"], first["color"], first["text"]), ((32, 5, 184, 18), "Body / Semibold", "color/text", "6 minutes ago"))
        self.assertEqual((second["abs"], second["style"], second["color"], second["text"]), ((32, 21, 184, 18), "Caption / Regular", "color/text-secondary", "Auto-saved"))

    def test_a_named_version_has_the_brand_dot_and_the_day_rows_are_tertiary_captions(self):
        named = fx.part(self.region, "Version 3")
        self.assertEqual((named["kids"][0]["fill"], fx.texts(named)), ("color/text-brand", ["Before the colour pass", "55 minutes ago"]))
        day = fx.part(self.region, "Day 1")
        text = fx.part(day, "Text")
        self.assertEqual((fx.texts(day), text["style"], text["color"], text["abs"]), (["Today"], "Caption / Semibold", "color/text-tertiary", (8, 7, 208, 18)))

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
