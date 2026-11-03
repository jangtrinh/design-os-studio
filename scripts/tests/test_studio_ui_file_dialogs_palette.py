"""Dialog/CommandPalette against the painters 0.7.5 draws (synthetic env: names are the theme constants; the looks come from the real kit-states.json).

Surface: `kit::command_bar_surface` (widgets/kit/surface.rs:71-74, `command_look` :41-44). CmdK: `cmdk.rs:283-604` `show` (bar 440, search row 40, tab row 34, section bands 28, rows pitch 36).
Actions: `panels/menus.rs:918-1086` `palette` (520 wide, search row `CONTROL_MD`, tab row 34, section band 22, rows 36). The captures `j-cmdk` (440 x 320) and `j-palette` (520 x 394)
fix two sizes: the oracle in the fixtures lays the built frames out with the production's own numbers and must land on both.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_dialogs_palette
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import rest_ir as ir  # noqa: E402
from components import dialogs_palette  # noqa: E402
from studio_ui_palette_fixtures import ENV, SETS, STYLES, build, flow_h, lit_of, named  # noqa: E402


class RecordTest(unittest.TestCase):
    def test_the_set_is_the_two_registry_kinds_in_the_dialogs_row_with_a_placeholder_and_a_body_slot(self):
        s = SETS["Dialog/CommandPalette"]
        self.assertEqual((s["row"], s["axes"], s.get("uses", [])), ("Dialogs", {"Kind": ["CmdK", "Actions"], "State": ["Default"]}, []))
        self.assertEqual(s["props"], [{"name": "Placeholder", "type": "TEXT", "default": "Search..."}, {"name": "Body", "type": "SLOT"}])

    def test_both_kinds_are_the_command_bar_surface_and_nest_no_set_and_draw_no_focus(self):
        for kind, w in (("CmdK", 440.0), ("Actions", 520.0)):
            bar = build(kind)
            self.assertEqual((bar["fill"], bar["stroke"], bar["sw"], bar["radius"], bar["w"], bar["sx"], bar["sy"]), ("BG", "BORDER_MENU", "BORDER_WIDTH", "RADIUS_XL", w, "fixed", "hug"), kind)
            self.assertEqual([k["name"] for k in ir.walk(bar) if k["t"] == "X"], [], f"{kind}: no nested instance, so no Focus variant and no Menu/Item")

    def test_the_hairlines_are_local_border_menu_lines_never_divider_hairline(self):
        for kind in ("CmdK", "Actions"):
            lines = [k for k in ir.walk(build(kind)) if k["name"] == "Line"]
            self.assertTrue(lines, kind)
            self.assertEqual({k["fill"] for k in lines}, {"BORDER_MENU"}, kind)

    def test_every_text_uses_a_defined_text_style_and_every_pinned_literal_names_its_rust_line(self):
        for kind in ("CmdK", "Actions"):
            for k in ir.walk(build(kind)):
                if k["t"] == "T":
                    self.assertIn(k["style"], STYLES, kind)
                for e in (ir.pads(k) if k["t"] == "F" else []) + [k.get("gap"), k.get("radius")]:
                    if isinstance(e, tuple):
                        self.assertRegex(e[2], r"\w+\.rs:\d+", f"{kind}: lit({e[1]})")


class CmdKTest(unittest.TestCase):
    def test_the_search_row_is_40_high_with_the_hairline_on_its_last_pixel(self):
        row = named(build("CmdK"), "Search row")[0]
        self.assertEqual((row["h"], row["sx"]), (40.0, "fill"))
        self.assertEqual(named(row, "Line")[0]["abs"], (0.0, 39.0, 440.0, 1.0), "cmdk.rs:375: hline at input_row.bottom() - 1, the capture's pixel row 39")

    def test_the_search_icon_is_15_px_centred_at_x_18_in_icon_secondary(self):
        icon = named(build("CmdK"), "Search icon")[0]
        self.assertEqual((icon["icon"], icon["size"], icon["color"], icon["abs"]), ("Search", 15, "ICON_SECONDARY", (10.5, 12.5, 15.0, 15.0)))

    def test_the_placeholder_is_a_local_body_text_in_a_365_px_clipped_field_not_input_search(self):
        row = named(build("CmdK"), "Search row")[0]
        field = named(row, "Field")[0]
        self.assertEqual((field["w"], field["sx"], field.get("clip")), (365.0, "fixed", True), "the capture's unnamed rect (34, 8, 365, 15)")
        text = named(field, "Placeholder")[0]
        self.assertEqual((text["style"], text["color"], text["prop"]), ("Body / Regular", "TEXT_PLACEHOLDER", "Placeholder"))
        self.assertEqual([lit_of(p) for p in ir.pads(row)], [("lit", 6.5), None, None, ("lit", 34.0)])

    def test_the_image_button_is_a_26_px_frame_with_no_fill_and_a_14_px_glyph_not_button_icon(self):
        btn = named(build("CmdK"), "Image button")[0]
        self.assertEqual((btn["abs"], btn.get("fill")), ((407.0, 7.0, 26.0, 26.0), None), "cmdk.rs:379: centred at right - 20; a fill only on hover")
        self.assertEqual((btn["sx"], btn["sy"], btn.get("align", ("center", "center"))), ("fixed", "fixed", ("center", "center")), "a hugging frame would shrink to the 14 px glyph and put it at the corner")
        glyph = named(btn, "Glyph")[0]
        self.assertEqual((glyph["icon"], glyph["size"], glyph["color"]), ("Image", 14, "ICON_SECONDARY"))

    def test_the_tab_row_is_34_high_with_fixed_tabs_12_apart_from_x_14_and_a_hairline_on_row_33(self):
        row = named(build("CmdK"), "Tab row")[0]
        self.assertEqual((row["h"], row["sx"], [lit_of(p) for p in ir.pads(row)], lit_of(row["gap"])), (34.0, "fill", [None, None, None, ("lit", 14.0)], ("lit", 12.0)))
        tabs = [k for k in row["kids"] if k["name"].startswith("Tab ")]
        self.assertEqual([t["w"] for t in tabs], [23.0, 47.75, 110.75, 78.5], "text_width(Body Semibold) + 8 for every tab, the capture's unnamed rects")
        self.assertEqual([named(t, "Label")[0]["text"] for t in tabs], ["All", "Assets", "Plugins & widgets", "Design Gate"])
        self.assertEqual(named(row, "Line")[0]["abs"], (0.0, 33.0, 440.0, 1.0), "cmdk.rs:420: hline at tab_row.bottom() - 1, the capture's pixel row 73")
        self.assertEqual(row["kids"][0]["name"], "Line", "drawn first: the active tab's underline (cmdk.rs:441) paints over it on row 73")

    def test_only_the_active_tab_is_semibold_text_with_a_2_px_brand_underline_inset_2(self):
        tabs = [k for k in named(build("CmdK"), "Tab row")[0]["kids"] if k["name"].startswith("Tab ")]
        active, rest = tabs[0], tabs[1:]
        label, line = named(active, "Label")[0], named(active, "Underline")[0]
        self.assertEqual((label["style"], label["color"]), ("Body / Semibold", "TEXT"))
        self.assertEqual((line["fill"], line["abs"]), ("BORDER_SELECTED", (2.0, 32.0, 19.0, 2.0)), "cmdk.rs:441: tr.left + 2 .. tr.right - 2, the bottom 2 px")
        for t in rest:
            self.assertEqual([(l["style"], l["color"]) for l in named(t, "Label")], [("Body / Regular", "TEXT_QUATERNARY")])
            self.assertEqual(named(t, "Underline"), [])

    def test_the_body_is_a_slot_with_the_captures_two_sections_and_five_rows_the_first_selected(self):
        body = named(build("CmdK"), "Body")[0]
        self.assertEqual((body["slot"], body["sx"], body["dir"], body["align"]), ("Body", "fill", "V", ("start", "center")))
        self.assertEqual([k["name"] for k in body["kids"]], ["Section", "Row 1", "Row 2", "Section 2", "Row 3", "Row 4", "Row 5"])
        self.assertEqual(([lit_of(p) for p in ir.pads(body)], lit_of(body["gap"])), ([("lit", 4.0), None, ("lit", 10.0), None], ("lit", 4.0)))
        section = body["kids"][0]
        caption = named(section, "Caption")[0]
        self.assertEqual((section["h"], section["sx"], [lit_of(p) for p in ir.pads(section)]), (24.0, "fill", [("lit", 6.0), None, None, ("lit", 16.0)]))
        self.assertEqual((caption["style"], caption["color"]), ("Caption / Regular", "TEXT_TERTIARY"))

    def test_a_row_is_a_424_by_32_local_frame_in_the_command_row_look_not_menu_item(self):
        first, second = named(build("CmdK"), "Row 1")[0], named(build("CmdK"), "Row 2")[0]
        for row in (first, second):
            self.assertEqual((row["w"], row["h"], row["radius"], row["sx"]), (424.0, 32.0, "RADIUS_CONTROL", "fixed"))
            self.assertEqual(([lit_of(p) for p in ir.pads(row)]), [None, ("lit", 8.0), None, ("lit", 32.0)])
        self.assertEqual((first["fill"], named(first, "Title")[0]["color"], named(first, "Icon")[0]["color"]), ("BG_SELECTED", "TEXT", "ICON"))
        self.assertEqual((second.get("fill"), named(second, "Title")[0]["color"], named(second, "Icon")[0]["color"]), (None, "TEXT_SECONDARY", "ICON_TERTIARY"))
        icon = named(first, "Icon")[0]
        self.assertEqual((icon["icon"], icon["size"], icon["abs"]), ("Dev", 14, (7.0, 9.0, 14.0, 14.0)), "cmdk.rs:494: 14 px centred at x 14 of the row")
        self.assertEqual((named(first, "Title")[0]["style"], named(first, "Title")[0]["sx"]), ("Body / Regular", "fill"))

    def test_the_count_badge_is_a_local_neutral_pill_padded_6_a_side_because_status_count_sm_pads_4(self):
        badge = named(build("CmdK"), "Badge")[0]
        self.assertEqual((badge["fill"], badge["stroke"], badge["h"], badge["sx"]), ("BADGE_NEUTRAL_BG", "BADGE_NEUTRAL_BORDER", 22.0, "hug"))
        self.assertEqual(([lit_of(p) for p in ir.pads(badge)], lit_of(badge["radius"])), ([None, ("lit", 6.0), None, ("lit", 6.0)], ("lit", 11.0)))
        label = named(badge, "Label")[0]
        self.assertEqual((label["style"], label["color"]), ("Text xs / Medium", "BADGE_NEUTRAL_TEXT"))

    def test_the_default_instance_is_the_captures_320_high(self):
        """j-cmdk cmdk-modal (500, 512, 440, 320): 74 + 2 x 28 + 5 x 36 + 10. The default Slot is the capture's content, so the default instance matches the capture 1:1."""
        self.assertEqual(flow_h(build("CmdK")), 74.0 + 2 * 28.0 + 5 * 36.0 + 10.0)
        self.assertEqual(flow_h(build("CmdK")), 320.0)

    def test_the_default_rows_are_the_captures_in_order_with_their_icons_and_count_badges(self):
        body = named(build("CmdK"), "Body")[0]
        self.assertEqual([named(k, "Caption")[0]["text"] for k in body["kids"] if k["name"].startswith("Section")], ["Recents", "Image editing"])
        rows = [k for k in body["kids"] if k["name"].startswith("Row ")]
        self.assertEqual([(named(r, "Title")[0]["text"], named(r, "Icon")[0]["icon"], named(r, "Label")[0]["text"], r.get("fill")) for r in rows],
                         [("Ease Design Figma Agent", "Dev", "Development", "BG_SELECTED"), ("Replace content", "Sparkle", "AI", None), ("Insert example card", "Image", "AI", None),
                          ("Boost resolution", "Sparkle", "AI", None), ("Edit image with prompt", "Sparkle", "AI", None)])


if __name__ == "__main__":
    unittest.main()
