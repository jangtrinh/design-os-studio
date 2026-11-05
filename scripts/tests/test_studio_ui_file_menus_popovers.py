"""Menu/Panel, Menu/Context and Popover/Panel against the painters 0.7.5 calls (synthetic env: names are the theme constants; looks from the real kit-states.json).

Menu panel: `popup_tree_with_row_h` (widgets/menu.rs:321-573), surface `kit::menu_surface` (widgets/kit/surface.rs:56-58), rows in `Layout::new` (menu.rs:166-186, 8 px of padding top
and bottom, `SEP_H` between rows). Context menu: `panels/context.rs:145-167` `show` (the same painter at a 230 minimum). Popover: `widgets/popover.rs:17-50` `show`, surface `kit::popover_surface`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_menus_popovers
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
from components import popover_kinds  # noqa: E402
import rest_ir as ir  # noqa: E402
import text_styles_plan  # noqa: E402
from components.env import KIT_STATE, KIT_STATES_REL  # noqa: E402

ROWS = json.loads((HERE.parents[1] / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]
NUM = {"MENU_WIDTH": 200.0, "MENU_PILL_INSET": 8.0, "MENU_ITEM_HEIGHT": 32.0, "CONTROL_MD": 40.0, "BORDER_WIDTH": 1.0}


class FakeEnv:
    def c(self, const):
        return const

    def num(self, const):
        return NUM.get(const, 1.0)

    def look(self, row, state):
        cell = ROWS[row][KIT_STATE[state]]
        out = {k: cell.get(k) for k in ("fill", "border", "text", "icon", "ring")}
        out["alpha"] = cell["alpha"]
        return out


SETS = {s["name"]: s for s in components.discover()}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def build(name, **values):
    return SETS[name]["build"](values or {"State": "Default"}, FakeEnv())


def named(node, name):
    return [k for k in ir.walk(node) if k["name"] == name]


class MenuPanelTest(unittest.TestCase):
    def test_the_set_is_one_default_variant_with_a_body_slot_nesting_item_and_separator(self):
        s = SETS["Menu/Panel"]
        self.assertEqual((s["row"], s["axes"], sorted(s["uses"])), ("Menus & Popovers", {"State": ["Default"]}, ["Menu/Item", "Menu/Separator"]))
        self.assertEqual(s["props"], [{"name": "Body", "type": "SLOT"}])

    def test_the_surface_is_bg_with_a_border_menu_edge_at_the_frame_radius_and_the_menu_width(self):
        panel = build("Menu/Panel")
        self.assertEqual((panel["fill"], panel["stroke"], panel["sw"], panel["radius"]), ("BG", "BORDER_MENU", "BORDER_WIDTH", "RADIUS_FRAME"))
        self.assertEqual((panel["w"], panel["sx"], panel["sy"]), (200.0, "fixed", "hug"))

    def test_the_rows_start_menu_padding_from_the_top_and_leave_it_at_the_bottom_with_no_side_padding(self):
        panel = build("Menu/Panel")
        self.assertEqual(ir.pads(panel), ["MENU_PADDING", None, "MENU_PADDING", None], "Layout::new `y = MENU_PADDING`; the pill inset is each Menu/Item's own 8 px")
        self.assertEqual([k["name"] for k in panel["kids"]], ["Body"])

    def test_the_body_is_the_slot_container_centred_so_pills_keep_8_px_and_separators_span_the_panel(self):
        body = named(build("Menu/Panel"), "Body")[0]
        self.assertEqual((body["slot"], body["dir"], body["sx"], body["align"]), ("Body", "V", "fill", ("start", "center")))
        items = [k for k in body["kids"] if k["set"] == "Menu/Item"]
        seps = [k for k in body["kids"] if k["set"] == "Menu/Separator"]
        self.assertEqual(len(items), 3)
        self.assertEqual([k["variant"] for k in items], ["Size=Xs, State=Default"] * 3)
        self.assertEqual([k["sx"] for k in seps], ["fill"] * 2, "menu.rs:419 hline(rect.left, rect.right): the line spans the panel, not the pill")
        self.assertEqual([k["set"] for k in body["kids"]], ["Menu/Item", "Menu/Separator", "Menu/Item", "Menu/Separator", "Menu/Item"])
        self.assertTrue(all("sx" not in k for k in items), "an item keeps its main's 184 px (200 - 2 * MENU_PILL_INSET): no resize")

    def test_the_default_rows_are_neutral_and_cover_the_icon_shortcut_and_submenu_parts(self):
        items = [k for k in named(build("Menu/Panel"), "Body")[0]["kids"] if k["set"] == "Menu/Item"]
        self.assertEqual([k["name"] for k in items], ["Item 1", "Item 2", "Item 3"])
        self.assertEqual(items[0]["props"], {"Label": "Item", "Has Submenu": False})
        self.assertEqual(items[1]["props"], {"Label": "Item", "Has Icon": True, "Has Shortcut": True, "Has Submenu": False})
        self.assertEqual(items[2]["props"], {"Label": "Item"})


class MenuContextTest(unittest.TestCase):
    def test_the_set_is_menu_panel_at_the_230_minimum_with_the_same_slot(self):
        s, panel = SETS["Menu/Context"], build("Menu/Context")
        self.assertEqual((s["row"], s["axes"], sorted(s["uses"]), s["props"]), ("Menus & Popovers", {"State": ["Default"]}, ["Menu/Item", "Menu/Separator"], [{"name": "Body", "type": "SLOT"}]))
        self.assertEqual((panel["w"], panel["fill"], panel["stroke"], panel["radius"]), (230.0, "BG", "BORDER_MENU", "RADIUS_FRAME"))
        self.assertEqual(ir.pads(panel), ["MENU_PADDING", None, "MENU_PADDING", None])

    def test_its_pills_fill_the_width_between_the_pill_insets(self):
        body = named(build("Menu/Context"), "Body")[0]
        self.assertEqual(ir.pads(body), [None, "MENU_PILL_INSET", None, "MENU_PILL_INSET"], "230 - 2 * 8 = 214: a Menu/Item main is 184, so the rows fill")
        self.assertEqual([k["sx"] for k in body["kids"] if k["set"] == "Menu/Item"], ["fill"] * 3)

    def test_its_separators_are_as_wide_as_the_panel_not_the_pill(self):
        """`hline(rect.left, rect.right)` spans the panel's 230 (menu.rs:517): a fixed-width copy, centred by the body, overflows the 214 content box by the 8 px pill inset on each side."""
        body = named(build("Menu/Context"), "Body")[0]
        seps = [k for k in body["kids"] if k["set"] == "Menu/Separator"]
        self.assertEqual([(k["w"], k.get("sx")) for k in seps], [(230.0, None)] * 2)
        self.assertEqual(body["align"][1], "center")
        self.assertEqual([k.get("w") for k in named(build("Menu/Panel"), "Body")[0]["kids"] if k["set"] == "Menu/Separator"], [None] * 2, "Menu/Panel has no side padding: Fill is the panel's width")


class PopoverPanelTest(unittest.TestCase):
    def test_the_set_has_a_default_state_a_kind_axis_a_title_and_a_body_slot(self):
        s = SETS["Popover/Panel"]
        self.assertEqual((s["row"], s["axes"], sorted(s["uses"])), ("Menus & Popovers", {"State": ["Default"], "Kind": ["Default", *popover_kinds.ORDER]},
                                                                      ["Button/Close", "Button/Icon", "Button/Primary", "Checkbox/Box", "Divider/Hairline", "Input/Field", "Input/PropertyField",
                                                                       "Radio/Card", "Select/Trigger"]))
        self.assertEqual(s["props"], [{"name": "Title", "type": "TEXT", "default": "Settings"}, {"name": "Body", "type": "SLOT"}])

    def test_the_surface_is_the_popover_look_at_the_frame_radius(self):
        panel = build("Popover/Panel")
        self.assertEqual((panel["fill"], panel["stroke"], panel["sw"], panel["radius"], panel["w"]), ("BG", "BORDER_MENU", "BORDER_WIDTH", "RADIUS_FRAME", 240.0))

    def test_the_header_is_40_high_with_the_title_at_16_a_close_at_right_32_top_8_and_the_hairline_on_its_last_pixel(self):
        header = named(build("Popover/Panel"), "Header")[0]
        self.assertEqual((header["h"], header["sx"]), (40.0, "fill"))
        pad = ir.pads(header)
        self.assertEqual([p[:2] if p else None for p in pad], [None, None, None, ("lit", 16.0)])
        self.assertIn("popover.rs:9", pad[3][2], "LABEL_X is `pub const LABEL_X: f32 = 16.0`, a local constant: INSET_POPOVER only holds the same 16")
        title = named(header, "Title")[0]
        self.assertEqual((title["style"], title["color"], title["prop"]), ("Body / Semibold", "TEXT", "Title"))
        close, line = named(header, "Close")[0], named(header, "Hairline")[0]
        self.assertEqual((close["set"], close["variant"], close["abs"]), ("Button/Close", "Size=Xs, State=Default", (208.0, 8.0, 24.0, 24.0)))
        self.assertEqual((line["set"], line["variant"], line["abs"]), ("Divider/Hairline", "State=Default", (0.0, 39.0, 240.0, 1.0)))

    def test_the_body_is_a_slot_with_one_neutral_label_row_in_text_secondary(self):
        body = named(build("Popover/Panel"), "Body")[0]
        self.assertEqual((body["slot"], body["sx"], body["dir"]), ("Body", "fill", "V"))
        label = named(body, "Label")[0]
        self.assertEqual((label["style"], label["color"]), ("Body / Regular", "TEXT_SECONDARY"))
        row = named(body, "Row")[0]
        self.assertEqual(row["align"], ("start", "center"), "popover.rs:58 draws the label at body.left + LABEL_X: left-aligned, not centred")
        self.assertEqual([p[:2] if p else None for p in ir.pads(row)], [None, None, None, ("lit", 16.0)])
        self.assertIn("popover.rs:9", ir.pads(row)[3][2])

    def test_the_body_pads_8_above_and_below_as_the_raw_plus_8_of_popover_row(self):
        body = named(build("Popover/Panel"), "Body")[0]
        pad = ir.pads(body)
        self.assertEqual([p[:2] if p else None for p in pad], [("lit", 8.0), None, ("lit", 8.0), None])
        self.assertIn("popover.rs:54", pad[0][2], "`body.top() + 8.0` is a raw literal; INSET_FRAME only holds the same 8")

    def test_every_pinned_literal_names_the_rust_line_it_comes_from(self):
        for node in ir.walk(build("Popover/Panel")):
            for e in (ir.pads(node) if node["t"] == "F" else []):
                if isinstance(e, tuple):
                    self.assertRegex(e[2], r"\w+\.rs:\d+")


class EveryStyleExistsTest(unittest.TestCase):
    def test_every_text_the_sets_draw_uses_a_defined_text_style(self):
        for name in ("Menu/Panel", "Menu/Context", "Popover/Panel"):
            for t in (k for k in ir.walk(build(name)) if k["t"] == "T"):
                self.assertIn(t["style"], STYLES, name)


if __name__ == "__main__":
    unittest.main()
