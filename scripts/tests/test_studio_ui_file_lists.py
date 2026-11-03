"""List/Row and Tree/Row against the painters 0.7.5 calls (synthetic env: no theme.rs or MCP; the looks come from the real kit-states.json).

List/Row Sm: the Home sidebar row (home/mod.rs:1218-1246). List/Row Xs: the pages-list row (panels/left.rs:311-352) with `kit::focus_ring`.
Tree/Row: the layers row (panels/layers.rs:485-700), look `kit::tree_row_look`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_lists
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
NUM = {"CONTROL_SM": 36.0, "PAGE_ROW_HEIGHT": 32.0, "LAYER_ROW_HEIGHT": 32.0, "LAYER_CHEVRON_BOX": 24.0, "LAYER_TOGGLE_SIZE": 24.0}


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


def build(name, **values):
    """An axis the caller leaves out takes its first value, which is today's look (the first Depth is 0: no indent)."""
    return SETS[name]["build"]({**{a: v[0] for a, v in SETS[name]["axes"].items()}, **values}, FakeEnv())


def parts(node, t=None):
    return [k for k in ir.walk(node) if k is not node and (t is None or k["t"] == t)]


class ListRowTest(unittest.TestCase):
    def test_the_set_is_the_registry_matrix_in_the_lists_row(self):
        self.assertEqual((SETS["List/Row"]["row"], len(components.matrix(SETS["List/Row"]["axes"]))), ("Lists & Trees", 8))

    def test_sm_is_the_home_row_36_high_with_a_15_px_glyph_and_text_sm_regular(self):
        node = build("List/Row", Size="Sm", State="Default")
        self.assertEqual((node["h"], node["radius"]), (36.0, "RADIUS_CONTROL"))
        glyph, label = parts(node, "I")[0], parts(node, "T")[0]
        self.assertEqual((glyph["size"], glyph["swap"], label["style"], label["color"]), (15, "Icon", "Text sm / Regular", "TEXT_SECONDARY"))

    def test_sm_places_the_glyph_centre_at_26_and_the_label_at_42(self):
        """home/mod.rs:1235-1243: ix = left + 12, the glyph is centred at ix + 14 and the label starts at ix + 30: a 28 px glyph slot and a 2 px gap."""
        node = build("List/Row", Size="Sm", State="Default")
        slot = node["kids"][0]
        self.assertEqual(ir.pads(node)[3][:2], ("lit", 12.0))
        self.assertEqual((slot["w"], node["gap"]), (28, "INSET_SEGMENT"))
        self.assertEqual(12 + 28 / 2, 26)
        self.assertEqual(12 + 28 + 2, 42)

    def test_sm_selected_is_semibold_on_bg_selected_with_the_hovered_text_and_icon(self):
        node = build("List/Row", Size="Sm", State="Selected")
        glyph, label = parts(node, "I")[0], parts(node, "T")[0]
        self.assertEqual((node["fill"], label["style"], label["color"], glyph["color"]), ("BG_SELECTED", "Text sm / Semibold", "TEXT_SECONDARY_HOVER", "ICON_QUATERNARY_HOVER"))

    def test_sm_hover_is_bg_hover_and_regular(self):
        node = build("List/Row", Size="Sm", State="Hover")
        self.assertEqual((node["fill"], parts(node, "T")[0]["style"], parts(node, "T")[0]["color"]), ("BG_HOVER", "Text sm / Regular", "TEXT_SECONDARY_HOVER"))

    def test_sm_focus_is_the_default_look_because_the_home_row_paints_no_ring(self):
        self.assertEqual(build("List/Row", Size="Sm", State="Focus"), build("List/Row", Size="Sm", State="Default"))

    def test_xs_is_the_pages_row_with_no_glyph_and_body_text(self):
        node = build("List/Row", Size="Xs", State="Default")
        self.assertEqual((node["h"], parts(node, "I")), (32.0, []))
        label = parts(node, "T")[0]
        self.assertEqual((label["style"], label["color"]), ("Body / Regular", "TEXT_SECONDARY"))
        self.assertEqual(ir.pads(node), [None, None, None, "PAGE_ROW_TEXT_PAD"])
        self.assertEqual(build("List/Row", Size="Xs", State="Selected")["fill"], "BG_SELECTED")
        self.assertEqual(parts(build("List/Row", Size="Xs", State="Selected"), "T")[0]["style"], "Body / Semibold")
        self.assertEqual(build("List/Row", Size="Xs", State="Hover")["fill"], "BG_HOVER")

    def test_xs_focus_is_the_default_look_plus_a_ring_on_the_row(self):
        focus, default = build("List/Row", Size="Xs", State="Focus"), build("List/Row", Size="Xs", State="Default")
        rings = [k for k in focus["kids"] if k["t"] == "X"]
        self.assertEqual([(r["name"], r["set"], r["variant"], r["abs"]) for r in rings], [("Focus ring", "Focus/Ring", "Tone=Brand", (-4, -4, 232.0, 40.0))])
        self.assertEqual({**focus, "kids": []}, {**default, "kids": []})
        self.assertEqual(focus["kids"][:-1], default["kids"])
        self.assertIn("Focus/Ring", SETS["List/Row"]["uses"])
        self.assertEqual([k for k in ir.walk(build("List/Row", Size="Sm", State="Focus")) if k["t"] == "X"], [])


class TreeRowTest(unittest.TestCase):
    def test_the_set_is_the_three_registry_states_at_six_depths(self):
        self.assertEqual((SETS["Tree/Row"]["row"], len(components.matrix(SETS["Tree/Row"]["axes"]))), ("Lists & Trees", 18))

    def test_the_row_is_a_32_px_pill_with_no_padding_so_an_indent_can_override_the_left_one(self):
        node = build("Tree/Row", Size="Xs", State="Default")
        self.assertEqual((node["h"], node["radius"], node["w"]), (32.0, "RADIUS_CONTROL", 224))
        self.assertEqual(ir.pads(node), [None, None, None, None])

    def test_parts_are_a_24_chevron_box_a_16_glyph_a_body_name_and_a_caption_type(self):
        node = build("Tree/Row", Size="Xs", State="Default")
        box = node["kids"][0]
        self.assertEqual((box["w"], box["h"], box["kids"][0]["size"], box["kids"][0]["visible_prop"], box["kids"][0]["swap"]), (24.0, 24.0, 12, "Has Children", "Chevron"))
        icons = [i for i in parts(node, "I") if i["name"] == "Icon"]
        self.assertEqual((icons[0]["size"], icons[0]["swap"]), (16, "Icon"))
        self.assertEqual(ir.pads(node["kids"][1])[1][:2], ("lit", 6.0))
        name, kind = [t for t in parts(node, "T")]
        self.assertEqual((name["style"], name["color"], name["sx"], name["prop"]), ("Body / Regular", "TEXT_TERTIARY", "fill", "Name"))
        self.assertEqual((kind["style"], kind["color"], kind["prop"]), ("Caption / Regular", "TEXT_TERTIARY", "Type label"))

    def test_the_type_label_ends_2_px_before_the_two_toggle_boxes(self):
        """layers.rs:585-586,662: text_right = row right - (2 * 24 + 8) and the label is drawn at text_right - 2; the pill is 8 in from the row."""
        node = build("Tree/Row", Size="Xs", State="Default")
        slot, toggles = node["kids"][3], node["kids"][4]
        self.assertEqual(ir.pads(slot), [None, "INSET_SEGMENT", None, None])
        self.assertEqual((toggles["name"], toggles["w"], toggles["h"]), ("Toggles", 48.0, 24.0))
        self.assertEqual([k["w"] for k in toggles["kids"]], [24.0, 24.0])

    def test_looks_follow_tree_row_look(self):
        self.assertIsNone(build("Tree/Row", Size="Xs", State="Default").get("fill"))
        self.assertEqual(build("Tree/Row", Size="Xs", State="Selected")["fill"], "BG_SELECTED")
        self.assertEqual(build("Tree/Row", Size="Xs", State="Hover")["fill"], "BG_HOVER")
        self.assertEqual({i["color"] for i in parts(build("Tree/Row", Size="Xs", State="Selected"), "I")}, {"ICON_QUATERNARY"})

    def test_only_hover_draws_the_eye_and_lock_glyphs_the_boxes_stay_to_reserve_the_room(self):
        """No hidden glyph is kept in the other states: a hidden alternate is copied into every row of every screen (about 7.7 MB of the file)."""
        for state in ("Default", "Selected", "Hover"):
            node = build("Tree/Row", Size="Xs", State=state)
            toggles = [i for i in parts(node, "I") if i["name"].endswith("toggle")]
            self.assertEqual([(i["icon"], i["size"], i["color"], i.get("hidden", False)) for i in toggles],
                             [("Unlock", 14, "ICON_QUATERNARY", False), ("Eye", 14, "ICON_QUATERNARY", False)] if state == "Hover" else [], state)
            boxes = [k for k in parts(node, "F") if k["name"] in ("Lock toggle box", "Eye toggle box")]
            self.assertEqual([(k["w"], k["h"]) for k in boxes], [(24.0, 24.0)] * 2, state)
            self.assertEqual(len([k for k in node["kids"] if k["name"] == "Toggles"]), 1, state)

if __name__ == "__main__":
    unittest.main()
