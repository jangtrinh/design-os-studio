"""Dialog/CommandPalette Kind=Actions against `panels/menus.rs:918-1086` `palette` (520 wide, search row `CONTROL_MD`, tab row 34, section band 22, rows 36): the capture `j-palette`
(520 x 394) fixes the size, and the oracle in the fixtures lays the built frame out with the production's own numbers.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_dialogs_palette_actions
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import rest_ir as ir  # noqa: E402
from components import dialogs_palette  # noqa: E402
from studio_ui_palette_fixtures import ENV, build, flow_h, lit_of, named  # noqa: E402


class ActionsTest(unittest.TestCase):
    def test_the_search_row_is_40_high_with_a_16_px_search_a_14_px_dev_glyph_and_a_centred_placeholder(self):
        row = named(build("Actions"), "Search row")[0]
        self.assertEqual((row["h"], row["sx"], [lit_of(p) for p in ir.pads(row)], row["align"]), (40.0, "fill", [None, None, None, ("lit", 36.0)], ("start", "center")))
        search, dev = named(row, "Search icon")[0], named(row, "Dev icon")[0]
        self.assertEqual((search["icon"], search["size"], search["color"], search["abs"]), ("Search", 16, "ICON_SECONDARY", (12.0, 12.0, 16.0, 16.0)))
        self.assertEqual((dev["icon"], dev["size"], dev["color"], dev["abs"]), ("Dev", 14, "ICON_SECONDARY", (493.0, 13.0, 14.0, 14.0)))
        text = named(row, "Placeholder")[0]
        self.assertEqual((text["style"], text["color"], text["prop"]), ("Body / Regular", "TEXT_PLACEHOLDER", "Placeholder"))

    def test_the_hairlines_sit_below_the_search_row_and_below_the_tab_row_in_flow(self):
        panel = build("Actions")
        self.assertEqual([k["name"] for k in panel["kids"]], ["Search row", "Line", "Tab row", "Line", "Body"])
        for line in (k for k in panel["kids"] if k["name"] == "Line"):
            self.assertEqual((line["h"], line["sx"], line["sy"]), (1.0, "fill", "fixed"), "menus.rs:990 and :1015 draw the line at the row's bottom edge, not inside it")

    def test_the_tab_row_is_34_high_with_24_px_pills_6_apart_from_x_12_all_regular_text(self):
        row = named(build("Actions"), "Tab row")[0]
        self.assertEqual((row["h"], [lit_of(p) for p in ir.pads(row)], lit_of(row["gap"]), row["align"]), (34.0, [None, None, None, ("lit", 12.0)], ("lit", 6.0), ("start", "center")))
        pills = [k for k in row["kids"] if k["name"].startswith("Tab ")]
        self.assertEqual([p["w"] for p in pills], [34.0, 58.0, 120.5, 72.5], "text_width(Body Regular) + 20, the capture's unnamed rects")
        self.assertEqual({p["h"] for p in pills}, {24.0})
        self.assertEqual([named(p, "Label")[0]["text"] for p in pills], ["All", "Assets", "Plugins & widgets", "AI & Gate"])
        self.assertEqual({named(p, "Label")[0]["style"] for p in pills}, {"Body / Regular"}, "unlike CmdK the active tab is not Semibold (menus.rs:996, :1007)")

    def test_the_active_pill_has_text_and_a_2_px_brand_line_on_its_bottom_edge(self):
        pills = [k for k in named(build("Actions"), "Tab row")[0]["kids"] if k["name"].startswith("Tab ")]
        self.assertEqual((named(pills[0], "Label")[0]["color"], named(pills[0], "Underline")[0]["fill"], named(pills[0], "Underline")[0]["abs"]), ("TEXT", "BORDER_SELECTED", (0.0, 23.0, 34.0, 2.0)))
        for p in pills[1:]:
            self.assertEqual((named(p, "Label")[0]["color"], named(p, "Underline")), ("TEXT_QUATERNARY", []))

    def test_the_body_is_a_slot_with_a_22_px_section_band_and_the_captures_eight_36_px_rows(self):
        body = named(build("Actions"), "Body")[0]
        self.assertEqual((body["slot"], body["sx"], body["dir"]), ("Body", "fill", "V"))
        self.assertEqual([k["name"] for k in body["kids"]], ["Section"] + [f"Row {i}" for i in range(1, 9)])
        self.assertEqual(ir.pads(body), [None, None, "MENU_PADDING", None], "menus.rs:954: MENU_PADDING closes the panel")
        section = body["kids"][0]
        self.assertEqual((section["h"], [lit_of(p) for p in ir.pads(section)], named(section, "Caption")[0]["style"]), (22.0, [None, None, None, ("lit", 16.0)], "Caption / Regular"))

    def test_a_row_is_36_high_with_a_pill_inset_8_and_2_label_at_16_and_the_shortcut_16_from_the_right(self):
        body = named(build("Actions"), "Body")[0]
        first, second, fifth = body["kids"][1], body["kids"][2], body["kids"][5]
        for row in (first, second):
            self.assertEqual((row["h"], row["sx"], row["align"]), (36.0, "fill", ("start", "center")))
            self.assertEqual([lit_of(p) for p in ir.pads(row)], [None, "MENU_SHORTCUT_PAD", None, ("lit", 16.0)])
        pill = named(first, "Pill")[0]
        self.assertEqual((pill["fill"], pill["radius"], pill["abs"]), ("BG_SELECTED", "RADIUS_CONTROL", (8.0, 2.0, 504.0, 32.0)), "menus.rs:1044-1046: row.shrink2(8, 2)")
        self.assertEqual((named(first, "Label")[0]["color"], named(second, "Label")[0]["color"]), ("TEXT", "TEXT_SECONDARY"))
        self.assertEqual(named(second, "Pill"), [], "a rest row has no fill")
        shortcut = named(fifth, "Shortcut")[0]
        self.assertEqual((shortcut["style"], shortcut["color"]), ("Body / Regular", "TEXT_QUATERNARY"), "menus.rs:1060-1061: font_body, not Menu/Shortcut's Medium")
        self.assertEqual(named(first, "Shortcut"), [])

    def test_the_default_instance_is_the_captures_394_high(self):
        """j-palette overlay (460, 438, 520, 394): 40 + 1 + 34 + 1 + 22 + 8 x 36 + 8. The default Slot is the capture's content, so the default instance matches the capture 1:1."""
        self.assertEqual(flow_h(build("Actions")), 40.0 + 1 + 34 + 1 + 22 + 8 * 36 + 8)
        self.assertEqual(flow_h(build("Actions")), 394.0)
        rows = [("Row", None, True)] + [("Row", "Ctrl+O", False)] * 7
        self.assertEqual(flow_h(dialogs_palette.actions(ENV, rows)), 394.0, "the oracle and `actions` agree on any eight rows")

    def test_the_default_rows_are_the_captures_in_order_with_their_shortcuts_and_one_selected(self):
        body = named(build("Actions"), "Body")[0]
        self.assertEqual(named(body["kids"][0], "Caption")[0]["text"], "Recents")
        rows = body["kids"][1:]
        self.assertEqual([(named(r, "Label")[0]["text"], (named(r, "Shortcut") or [{}])[0].get("text"), bool(named(r, "Pill"))) for r in rows],
                         [("Back to files", None, True), ("New design file", None, False), ("New board", None, False), ("New deck", None, False), ("Open...", "Ctrl+O", False),
                          ("Import web capture...", None, False), ("Save", "Ctrl+S", False), ("Save as...", "Ctrl+Shift+S", False)])


if __name__ == "__main__":
    unittest.main()
