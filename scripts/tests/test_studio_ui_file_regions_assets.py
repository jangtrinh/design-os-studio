"""Panel/Assets of components/regions_sidepanels_assets.py (page `03 Studio`), read from the IR the builder returns against the real Semantic plan. Lines are the v0.7.5 tag
(`panels/assets.rs`); the capture is `k-assets`, the region is the left panel (49, 32, 240, 868), so every rect here is relative to that origin.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_assets
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio_ui_regions_fixtures as fx  # noqa: E402
from rest_ir import walk  # noqa: E402


class AssetsRegionTest(unittest.TestCase):
    def setUp(self):
        self.region = fx.build("Panel/Assets")
        self.rows = fx.part(self.region, "Components")["kids"]

    def test_it_is_one_plain_record_nesting_icon_search_card_button_and_hairline_sets_in_a_row_after_cards(self):
        spec = fx.SPECS["Panel/Assets"]
        self.assertEqual(spec["axes"], {})
        self.assertEqual(sorted(spec["uses"]), ["Button/Icon", "Button/Secondary", "Card/Library", "Divider/Hairline", "Input/Search"])
        self.assertEqual(spec["props"], [{"name": "Components", "type": "SLOT"}])
        rows = fx.components.STUDIO_ROWS
        self.assertGreater(rows.index(spec["row"]), rows.index("Cards"), "a nested instance needs its main lowered first: Card/Library is in the Cards row")

    def test_the_frame_is_the_left_panel_in_bg(self):
        r = self.region
        self.assertEqual((r["w"], r["h"], r["fill"], r.get("clip")), (240, 868, "color/bg", True))

    def test_the_header_is_a_large_semibold_title_at_the_panel_inset_and_the_libraries_button_8_from_the_right(self):
        title = fx.part(self.region, "Title")
        self.assertEqual((title["abs"], fx.texts(title), fx.part(title, "Text")["style"], title["pad"][3]), ((0, 0, 240, 40), ["Assets"], "Large / Semibold", "inset/panel"))
        libs = fx.part(self.region, "Libraries")
        self.assertEqual((libs["set"], libs["variant"], libs["props"], libs["abs"]), ("Button/Icon", "Size=Xs, State=Default", {"Icon": "BookOpen"}, (200, 4, 32, 32)))
        rules = [k for k in self.region["kids"] if k["t"] == "X" and k["set"] == "Divider/Hairline"]
        self.assertEqual([k["abs"] for k in rules], [(0, 40, 240, 1)], "assets.rs:443: hline(region.left(), region.right(), top + hdr_h, BORDER)")

    def test_the_search_row_is_search_filter_and_the_list_and_grid_toggle_with_the_list_selected(self):
        search = fx.part(self.region, "Search")
        self.assertEqual((search["set"], search["variant"], search["abs"], search["props"]), ("Input/Search", "Size=Xs, State=Default", (16, 48, 102, 32), {"Value": "Search all li…"}))
        flt, lst, grid = (fx.part(self.region, n) for n in ("Filter", "List view", "Grid view"))
        self.assertEqual([(k["variant"], k["props"]["Icon"], k["abs"]) for k in (flt, lst, grid)],
                         [("Size=Xs, State=Default", "StrokeSettings", (122, 48, 32, 32)), ("Size=Xs, State=Selected", "ListView", (158, 48, 32, 32)),
                          ("Size=Xs, State=Default", "GridView", (192, 48, 32, 32))])

    def test_the_category_pills_are_20_px_captions_the_selected_one_filled(self):
        pills = [fx.part(self.region, n) for n in ("All pill", "Local pill", "Used in file pill")]
        self.assertEqual([k["abs"] for k in pills], [(16, 86, 29, 20), (49, 86, 44.5, 20), (97.5, 86, 77.5, 20)])
        self.assertEqual([fx.texts(k) for k in pills], [["All"], ["Local"], ["Used in file"]])
        self.assertEqual([k.get("fill") for k in pills], ["color/bg-secondary", None, None])
        self.assertEqual([fx.part(k, "Text")["style"] for k in pills], ["Caption / Semibold", "Caption / Regular", "Caption / Regular"])
        self.assertEqual([fx.part(k, "Text")["color"] for k in pills], ["color/text", "color/text-secondary", "color/text-secondary"])

    def test_the_libraries_section_has_the_heading_the_library_card_and_the_add_button(self):
        head = fx.part(self.region, "Heading")
        self.assertEqual((head["abs"], fx.texts(head), fx.part(head, "Text")["style"]), ((16, 111, 208, 18), ["All libraries"], "Body / Semibold"))
        card = fx.part(self.region, "Library card")
        self.assertEqual((card["set"], card["variant"], card["abs"], card["props"]), ("Card/Library", "", (16, 134, 208, 86), {"Count": "12 components"}))
        add = fx.part(self.region, "Add libraries")
        self.assertEqual((add["t"], add["abs"], add["set"], add["variant"], add["w"]), ("X", (16, 226, 208, 32), "Button/Secondary", "Size=Xs, State=Default", 208))
        self.assertEqual(add["props"], {"Label": "Add more libraries"})

    def test_the_list_is_a_clipped_slot_from_y_266_of_32_px_group_rows_and_48_px_component_rows(self):
        slot = fx.part(self.region, "Components")
        self.assertEqual((slot["abs"], slot["slot"], slot["dir"], slot.get("clip")), ((0, 266, 240, 602), "Components", "V", True))
        self.assertEqual([k["h"] for k in self.rows], [32, 48, 48, 32, 48, 32, 48, 48, 48, 48, 32, 32, 48, 48])
        self.assertEqual(sum(k["h"] for k in self.rows), 592, "the capture's list: rows down to the panel's bottom edge, the last two cut")

    def test_a_component_row_is_a_40_px_preview_box_at_16_with_the_name_12_after_it(self):
        row = self.rows[1]
        box = fx.part(row, "Preview")
        self.assertEqual((box["abs"], box["fill"], box["stroke"], box["radius"]), ((16, 4, 40, 40), "color/bg-secondary", "color/border-translucent", "radius/control"))
        self.assertEqual(fx.lit_of(box["sw"]), ("lit", 1.0))
        self.assertEqual([(i["icon"], i["size"], i["color"]) for i in walk(box) if i["t"] == "I"], [("Component", 16, "color/canvas-component")])
        name = fx.part(row, "Name")
        self.assertEqual((name["abs"], name["style"], name["color"]), ((68, 15, 160, 18), "Body / Regular", "color/text"), "assets.rs:340: box.right + SPACE_GROUP")

    def test_a_container_row_is_a_tertiary_body_line_indented_8_more_than_the_panel_padding(self):
        for row in (self.rows[0], self.rows[11]):
            text = fx.part(row, "Name")
            self.assertEqual((text["abs"][0], text["style"], text["color"]), (24, "Body / Regular", "color/text-tertiary"), "assets.rs:642: paint_group_row(.., 8.0)")

    def test_the_scrollbar_thumb_is_the_minimum_24_px_at_the_top_of_the_list(self):
        thumb = fx.part(self.region, "Scrollbar thumb")
        self.assertEqual((thumb["abs"], thumb["fill"]), ((234, 266, 4, 24), "color/text-tertiary"))

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
