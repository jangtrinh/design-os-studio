"""Panel/CheckDesigns of components/regions_sidepanels_check.py (page `03 Studio`), read from the IR the builder returns against the real Semantic plan. Lines are the v0.7.5 tag
(`panels/check_designs.rs`); the capture is `j-check-designs` (identical to `check-designs-fixed`), the region is the popover (867, 44, 320, 364), so every rect here is relative to it.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_check_designs
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio_ui_regions_fixtures as fx  # noqa: E402
from rest_ir import walk  # noqa: E402


class CheckDesignsRegionTest(unittest.TestCase):
    def setUp(self):
        self.region = fx.build("Panel/CheckDesigns")
        self.rows = fx.part(self.region, "Findings")["kids"]

    def test_it_is_one_plain_record_nesting_the_buttons_badge_and_slot_free_parts_in_the_regions_row_after_cards(self):
        spec = fx.SPECS["Panel/CheckDesigns"]
        self.assertEqual(spec["axes"], {})
        self.assertEqual(sorted(spec["uses"]), ["Badge/StatusCount", "Button/Close", "Button/Icon", "Button/Primary"])
        self.assertEqual(spec["props"], [{"name": "Findings", "type": "SLOT"}])
        self.assertEqual(spec["row"], "Regions")

    def test_the_frame_is_the_popover_surface_320_by_364(self):
        r = self.region
        self.assertEqual((r["w"], r["h"], r["fill"], r["stroke"], r["sw"], r["radius"], r.get("clip")), (320, 364, "color/bg", "color/border-menu", "size/border-width", "radius/frame", True))

    def test_the_header_is_a_large_semibold_title_12_in_and_two_32_px_buttons_12_from_the_right(self):
        title = fx.part(self.region, "Title")
        self.assertEqual((title["abs"], fx.texts(title), fx.part(title, "Text")["style"], fx.lit_of(title["pad"][3])), ((0, 0, 320, 44), ["Check designs"], "Large / Semibold", ("lit", 12.0)))
        rules, close = fx.part(self.region, "Rules"), fx.part(self.region, "Close")
        self.assertEqual((rules["set"], rules["props"], rules["abs"]), ("Button/Icon", {"Icon": "Sliders"}, (240, 6, 32, 32)))
        self.assertEqual((close["set"], close["abs"]), ("Button/Close", (276, 6, 32, 32)), "check_designs.rs:165-170: centred on right - PAD - 16, SPACE_FACT between the two")

    def test_the_summary_has_the_scope_the_counts_and_a_disabled_primary_fix_all(self):
        scope, counts = fx.part(self.region, "Scope"), fx.part(self.region, "Counts")
        self.assertEqual((scope["abs"], fx.texts(scope), fx.part(scope, "Text")["style"], fx.part(scope, "Text")["color"]),
                         ((12, 44, 220, 28), ["Selection · 1 layer"], "Body / Medium", "color/text"))
        self.assertEqual((counts["abs"], fx.texts(counts), fx.part(counts, "Text")["color"]), ((12, 60, 220, 28), ["3 issues · 4 layers checked"], "color/text-secondary"))
        fix = fx.part(self.region, "Fix all")
        self.assertEqual((fix["set"], fix["variant"], fix["props"], fix["abs"]), ("Button/Primary", "Size=Xs, State=Disabled", {"Label": "Fix all"}, (245.25, 50, 62.75, 32)))

    def test_the_hairlines_are_border_subtle_not_divider_hairline(self):
        lines = [k for k in self.region["kids"] if k["name"].startswith("Line")]
        self.assertEqual([(k["abs"], k["fill"]) for k in lines], [((0, 44, 320, 1), "color/border-subtle"), ((0, 88, 320, 1), "color/border-subtle")])
        self.assertEqual([k for k in walk(self.region) if k["t"] == "X" and k["set"] == "Divider/Hairline"], [], "Divider/Hairline's main is BORDER; the panel's rules are BORDER_SUBTLE")

    def test_the_findings_are_a_clipped_slot_from_y_88_of_three_groups_and_three_60_px_rows(self):
        slot = fx.part(self.region, "Findings")
        self.assertEqual((slot["abs"], slot["slot"], slot["dir"], slot.get("clip")), ((0, 88, 320, 276), "Findings", "V", True))
        self.assertEqual([k["h"] for k in self.rows], [32, 60, 32, 60, 32, 60])
        self.assertEqual([k["name"].split()[0] for k in self.rows], ["Group", "Finding"] * 3)

    def test_a_group_header_is_a_secondary_semibold_title_and_a_22_px_count_badge_after_it(self):
        group = self.rows[0]
        title = fx.part(group, "Group title")
        self.assertEqual((title["text"], title["style"], title["color"], title["abs"][:2]), ("Padding", "Body / Semibold", "color/text-secondary", (12, 7)))
        badge = fx.part(group, "Count")
        self.assertEqual(badge["abs"], (67.47, 5, 22, 22), "check_designs.rs:404-410: PAD + the title's width + SPACE_INLINE, width max(text + 16, BADGE_SM)")
        self.assertEqual((badge["t"], badge["set"], badge["variant"], badge["props"], badge["w"]), ("X", "Badge/StatusCount", "Size=Sm, Tone=Neutral, State=Default", {"Label": "1"}, 22))

    def test_a_finding_row_has_the_warning_glyph_the_name_two_message_lines_the_rule_and_two_buttons(self):
        row = self.rows[1]
        glyph = fx.part(row, "Rule icon")
        self.assertEqual((glyph["icon"], glyph["size"], glyph["color"], glyph["abs"]), ("Variables", 14, "color/warning", (13, 9, 14, 14)))
        name = fx.part(row, "Name")
        self.assertEqual((name["text"], name["style"], name["color"], name["abs"][:2]), ("Frame 1", "Body / Medium", "color/text", (36, 7)))
        lines = [fx.part(row, n) for n in ("Message line 1", "Message line 2")]
        self.assertEqual([(t["style"], t["color"]) for t in lines], [("Body / Regular", "color/text-secondary")] * 2)
        self.assertEqual(fx.part(row, "Rule")["abs"], (12, 59, 296, 1))
        ignore, dismiss = fx.part(row, "Ignore"), fx.part(row, "Dismiss")
        self.assertEqual((ignore["set"], ignore["props"], ignore["abs"]), ("Button/Icon", {"Icon": "EyeOff"}, (242, 8, 32, 32)))
        self.assertEqual((dismiss["set"], dismiss["abs"]), ("Button/Close", (276, 8, 32, 32)))

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
