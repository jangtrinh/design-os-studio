"""Menu/Item against `popup_tree_with_row_h` (widgets/menu.rs:321-573), synthetic env: no theme.rs or MCP; the looks come from the real kit-states.json.

Row geometry is from the row's left edge (menu.rs:455-516); the pill (fill) is inset `MENU_PILL_INSET` (8) on both sides, so every offset below
is checked against the row's coordinates, read from the laid-out IR: check at 12, icon box at 12 (its glyph at 16), or after a check the box at 28 with its
24 px of content at 32 (glyph at 36), label at 16 / 40 / 32 / 60, shortcut ending 16 and the chevron box 8 from the right edge.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_menus
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import rest_ir as ir  # noqa: E402
from components.env import KIT_STATE, KIT_STATES_REL  # noqa: E402

ROWS = json.loads((HERE.parents[1] / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]
NUM = {"MENU_ITEM_HEIGHT": 32.0, "CONTROL_SM": 36.0, "MENU_WIDTH": 200.0, "MENU_PILL_INSET": 8.0, "MENU_ICON_BOX": 24.0, "MENU_CHECK_BOX": 16.0,
       "MENU_CHEVRON_BOX": 24.0, "MENU_ICON_SIZE": 16.0, "MENU_CHEVRON_SIZE": 12.0, "MENU_ICON_LABEL_GAP": 4.0}


class FakeEnv:
    def c(self, const):
        return const

    def num(self, const):
        return NUM[const]

    def look(self, row, state):
        cell = ROWS[row][KIT_STATE[state]]
        out = {k: cell.get(k) for k in ("fill", "border", "text", "icon", "ring")}
        out["alpha"] = cell["alpha"]
        return out


SET = {s["name"]: s for s in components.discover()}["Menu/Item"]


def build(size="Xs", state="Default"):
    return SET["build"]({"Size": size, "State": state}, FakeEnv())


def named(node, name):
    return [k for k in ir.walk(node) if k["name"] == name]


def value(v):
    """A padding entry as a number: None is 0, a name is its theme constant, a lit is its literal."""
    return 0.0 if v is None else v[1] if isinstance(v, tuple) else NUM[v]


def width(node):
    """Width of a laid-out node: a fixed width as set, an icon its size, a hug frame its visible children plus padding (the label fills, so it has no width here)."""
    if node["t"] == "I":
        return float(node["size"])
    if node.get("sx") == "fixed" or node["t"] == "T":
        return float(node.get("w", 0.0))
    kids = [k for k in node["kids"] if not k.get("hidden")]
    pads = ir.pads(node)
    return sum(width(k) for k in kids) + value(pads[1]) + value(pads[3])


def place(node, left=0.0):
    """{name: x} of every visible descendant of an H auto-layout node whose left edge is at `left`, in the node's own coordinates (padding, then children in order;
    a fixed-width frame centres its content, as the root's `start` alignment and the hug frames do not)."""
    pads = ir.pads(node)
    kids = [k for k in node["kids"] if not k.get("hidden")]
    inner = left + value(pads[3])
    room = width(node) - value(pads[1]) - value(pads[3])
    total = sum(width(k) for k in kids)
    x = inner + ((room - total) / 2 if node.get("sx") == "fixed" and node.get("align", ("center", "center"))[0] == "center" else 0.0)
    out = {}
    for kid in kids:
        out[kid["name"]] = x
        if kid.get("kids"):
            out.update(place(kid, x))
        x += width(kid)
    return out


def shown(node, *names):
    """A copy of the row with the hidden parts `names` switched on (their boolean property set)."""
    node = ir.clone(node)
    for k in ir.walk(node):
        if k["name"] in names:
            k["hidden"] = False
    return node


def right_gap(node, name):
    """Distance from the row's right edge to the right edge of the part `name` when it is the last visible part (the label fills the space before it):
    the pill's own right inset, the root's right padding, then the part's right padding."""
    kids = [k for k in node["kids"] if not k.get("hidden") or k["name"] == name]
    last = [k for k in kids if k["name"] != "Submenu box"][-1]
    assert last["name"] == name, (last["name"], name)
    return NUM["MENU_PILL_INSET"] + value(ir.pads(node)[1]) + value(ir.pads(last)[1])


def row_x(node, name):
    """x of the part `name` from the row's left edge: the pill starts `MENU_PILL_INSET` in (menu.rs:467-470)."""
    return NUM["MENU_PILL_INSET"] + place(node)[name]


class MenuItemTest(unittest.TestCase):
    def test_the_set_is_the_registry_matrix_in_the_menus_row(self):
        self.assertEqual((SET["row"], len(components.matrix(SET["axes"]))), ("Menus & Overlays", 12))

    def test_xs_is_the_menu_item_height_and_sm_the_control_sm_height_the_home_menus_pass(self):
        self.assertEqual((build("Xs")["h"], build("Sm")["h"]), (32.0, 36.0))
        self.assertEqual((build()["w"], build()["radius"]), (184.0, "RADIUS_CONTROL"))

    def test_the_label_is_body_medium_and_fills_the_row(self):
        label = named(build(), "Label")[0]
        self.assertEqual((label["style"], label["sx"], label["prop"]), ("Body / Medium", "fill", "Label"))

    def test_a_plain_row_puts_the_label_at_16_an_icon_row_at_40(self):
        """The icon slot is its 24 px box plus MENU_ICON_LABEL_GAP of right padding: with the box hidden only the 4 px remain."""
        node = build()
        slot, box = named(node, "Icon slot")[0], named(node, "Icon box")[0]
        self.assertEqual((ir.pads(node)[3][:2], ir.pads(slot)[1]), (("lit", 4.0), "MENU_ICON_LABEL_GAP"))
        self.assertEqual((box["w"], box["visible_prop"], box.get("hidden")), (24.0, "Has Icon", True))
        self.assertEqual(named(node, "Icon")[0]["size"], 16)
        self.assertEqual(row_x(node, "Label"), 16)
        with_icon = shown(node, "Icon box")
        self.assertEqual((row_x(with_icon, "Icon box"), row_x(with_icon, "Icon"), row_x(with_icon, "Label")), (12, 16, 40))

    def test_selected_puts_the_check_at_12_and_the_label_at_32_or_60(self):
        """menu.rs:252-259 `label_x` after a check: MENU_LABEL_X_CHECK (32), 60 with an icon too; the icon box sits at MENU_ICON_X_CHECKED (32), its glyph
        centred in the 24 px box at 36. The 4 px gap after the check hides with the icon, so the label is at 32 without one."""
        for size in ("Xs", "Sm"):
            node = build(size, "Selected")
            check = named(node, "Check slot")[0]
            self.assertEqual((check["kids"][0]["icon"], check["kids"][0]["size"]), ("Check", 16), size)
            self.assertEqual((row_x(node, "Check slot"), row_x(node, "Check")), (12, 12), size)
            self.assertEqual(row_x(node, "Label"), 32, size)
            with_icon = shown(node, "Icon box")
            self.assertEqual((row_x(with_icon, "Icon box"), row_x(with_icon, "Icon"), row_x(with_icon, "Label")), (28, 36, 60), size)
        for state in ("Default", "Hover", "Active", "Focus", "Disabled"):
            self.assertEqual(named(build(state=state), "Check slot"), [], state)

    def test_looks_follow_menu_row_look(self):
        want = {"Default": (None, "TEXT_SECONDARY", "ICON_QUATERNARY"), "Hover": ("BG_HOVER", "TEXT_SECONDARY_HOVER", "ICON_QUATERNARY_HOVER"),
                "Selected": (None, "TEXT_SECONDARY", "ICON_BRAND_PRIMARY"), "Disabled": (None, "TEXT_SECONDARY", "ICON_QUATERNARY")}
        for state, (fill, text, icon) in want.items():
            node = build(state=state)
            self.assertEqual((node.get("fill"), named(node, "Label")[0]["color"], named(node, "Icon")[0]["color"]), (fill, text, icon), state)

    def test_hover_active_and_focus_are_one_look(self):
        hover = build(state="Hover")
        self.assertEqual(build(state="Active"), hover)
        self.assertEqual(build(state="Focus"), hover)
        self.assertEqual(named(hover, "Focus ring"), [])

    def test_disabled_halves_every_paint_and_has_no_fill(self):
        node = build(state="Disabled")
        self.assertIsNone(node.get("fill"))
        paints = [k for k in ir.walk(node) if k["t"] in ("T", "I")]
        self.assertTrue(paints)
        self.assertEqual({p.get("alpha") for p in paints}, {0.5})
        self.assertEqual({p.get("alpha", 1.0) for p in ir.walk(build()) if p["t"] in ("T", "I")}, {1.0})

    def test_the_hint_is_a_nested_menu_shortcut_in_the_row_state_and_ends_16_from_the_row_edge(self):
        """The hint is `kit::shortcut_look` (menu.rs:504-512): an instance of Menu/Shortcut, Disabled for a Disabled row and Default otherwise; the slot frame keeps the
        8 px of right padding that brings it to 16 from the row edge, and `Has Shortcut` shows it."""
        slot, hint = named(build(), "Shortcut slot")[0], named(build(), "Shortcut")[0]
        self.assertEqual((hint["t"], hint["set"], hint["variant"], hint["props"]), ("X", "Menu/Shortcut", "State=Default", {"Keys": "Ctrl+K"}))
        self.assertEqual((slot["visible_prop"], slot.get("hidden"), ir.pads(slot)[1], slot["kids"]), ("Has Shortcut", True, "MENU_PILL_INSET", [hint]))
        self.assertEqual(right_gap(build(), "Shortcut slot"), 16)
        self.assertEqual(named(build(state="Disabled"), "Shortcut")[0]["variant"], "State=Disabled")
        self.assertEqual({named(build(state=s), "Shortcut")[0]["variant"] for s in ("Default", "Selected", "Hover", "Active", "Focus")}, {"State=Default"})
        self.assertIn("Menu/Shortcut", SET["uses"])

    def test_the_keys_live_in_the_nested_instance_so_the_set_has_no_shortcut_text_property(self):
        """The importer binds `componentPropertyReferences` per node, so a Menu/Item text property cannot drive the nested `Keys` text: the use sets that text on the instance."""
        self.assertEqual([p["name"] for p in SET["props"]], ["Label", "Has Icon", "Has Shortcut", "Has Submenu", "Show check column", "Icon"])
        self.assertEqual([t for t in ir.walk(build()) if t["t"] == "T" and t.get("prop") == "Shortcut"], [])

    def test_the_submenu_chevron_is_a_24_box_flush_with_the_pill_and_a_12_px_glyph(self):
        box = named(build(), "Submenu box")[0]
        self.assertEqual((box["w"], box["h"], box["visible_prop"], box.get("hidden"), box["kids"][0]["size"], box["kids"][0]["icon"]),
                         (24.0, 24.0, "Has Submenu", None, 12, "ChevronRight"))
        self.assertEqual(ir.pads(build())[1], None)


if __name__ == "__main__":
    unittest.main()
