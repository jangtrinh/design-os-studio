"""Popover/Panel `Kind`, Menu/Item `Show check column` and Tabs/Segmented `Show cell 3` (synthetic env: names are the theme constants; looks from the real kit-states.json).

Each Kind is one production popover drawn at the size its 0.7.5 capture shows: widgets/popover.rs:17-65 (header 40, rows from 8 below it at a 32 px pitch, `popover::height`),
panels/typography.rs:596 (312 x 728), image_jobs.rs:1334 and :1440 (236 x 156, 248 x 144), comments/thread.rs:122 (300 x 212), agent_approval_card.rs:205 (360 x 248), skills/mod.rs:816
(320 x 192), panels/prototype.rs:518 (240 x 306).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b3_kinds
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
from components import popover_kinds, popover_kinds_shell  # noqa: E402
from components.env import KIT_STATE, KIT_STATES_REL  # noqa: E402

REGISTRY = json.loads((HERE.parents[1] / "design/component-registry.json").read_text(encoding="utf-8"))
ROWS = json.loads((HERE.parents[1] / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]
NUM = {"BORDER_WIDTH": 1.0, "LINE_CAPTION": 18.0, "SECTION_TRAILING_X": 208.0, "MENU_CHECK_BOX": 16.0, "MENU_ICON_LABEL_GAP": 4.0, "MENU_ICON_BOX": 24.0}
SIZES = {"EditStyleColor": (240, 184), "EditStyleEffect": (240, 344), "EditStyleGrid": (240, 344), "EditStyleText": (240, 280), "Stroke": (240, 216), "Noise": (240, 216), "Glass": (240, 248),
         "Texture": (240, 152), "TypeSettings": (312, 728), "Interaction": (240, 306), "Erase": (248, 144), "Expand": (236, 156), "CommentThread": (300, 212), "Skills": (320, 192),
         "Permissions": (360, 248)}
SETTINGS_ROWS = {"EditStyleColor": 4, "EditStyleEffect": 9, "EditStyleGrid": 9, "EditStyleText": 7, "Stroke": 5, "Noise": 5, "Glass": 6, "Texture": 3}


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
REGISTRY_KINDS = [v.partition("=")[2] for rec in REGISTRY["components"] if rec["name"] == "Popover/Panel" for v in rec["variants"] if v.startswith("Kind=")]


def popover(kind):
    return SETS["Popover/Panel"]["build"]({"State": "Default", "Kind": kind}, FakeEnv())


def named(node, name):
    return [k for k in ir.walk(node) if k["name"] == name]


class KindRegistryTest(unittest.TestCase):
    def test_every_registry_kind_after_default_has_a_builder_in_the_registry_order(self):
        self.assertEqual(REGISTRY_KINDS[0], "Default")
        self.assertEqual(list(popover_kinds.ORDER), REGISTRY_KINDS[1:])
        self.assertEqual(list(popover_kinds.KINDS), REGISTRY_KINDS[1:])

    def test_the_set_axes_equal_the_registry_and_default_is_first(self):
        spec = SETS["Popover/Panel"]
        self.assertEqual(spec["axes"], {"State": ["Default"], "Kind": REGISTRY_KINDS})
        components.check_specs([spec], REGISTRY)
        self.assertEqual(spec["axes"]["Kind"][0], "Default")

    def test_default_keeps_its_240_px_popover_and_its_paths(self):
        node = SETS["Popover/Panel"]["build"]({"State": "Default", "Kind": "Default"}, FakeEnv())
        self.assertEqual(node["w"], 240.0)
        for path in (("Header", "Title"), ("Header", "Close"), ("Header", "Hairline"), ("Body", "Row", "Label")):
            cur = node
            for name in path:
                cur = named(cur, name)[0]
        self.assertEqual(SETS["Popover/Panel"]["build"]({"State": "Default"}, FakeEnv())["w"], 240.0)  # a placement with only State resolves to the same drawing

class KindGeometryTest(unittest.TestCase):
    def test_each_kind_is_drawn_at_the_size_its_capture_shows(self):
        for kind, size in SIZES.items():
            node = popover(kind)
            self.assertEqual((node["w"], node["h"]), tuple(float(v) for v in size), kind)
            self.assertEqual((node["sx"], node["sy"]), ("fixed", "fixed"), kind)

    def test_a_settings_popover_is_the_popover_height_of_its_rows_with_a_row_every_32_px(self):
        for kind, rows in SETTINGS_ROWS.items():
            node = popover(kind)
            self.assertEqual(node["h"], popover_kinds_shell.height(rows), kind)
            self.assertEqual(node["h"], 40 + 8 + rows * 32 + 8, kind)
            tops = [k["abs"][1] - 3 for k in node["kids"] if k["t"] == "T" and k["name"].endswith(" label")]
            self.assertEqual(tops, [48.0 + 32 * i for i in range(rows)], kind)

    def test_a_row_is_no_frame_of_its_own_so_no_kind_pays_a_node_per_row(self):
        for kind in SETTINGS_ROWS:
            self.assertEqual([k["name"] for k in popover(kind)["kids"] if k["t"] == "F" and k["name"] in ("Row", "Body", "Header")], [], kind)

    def test_the_controls_sit_in_the_112_px_column_at_x_112(self):
        for kind in SETTINGS_ROWS:
            node = popover(kind)
            labels = [k["name"][:-6] for k in node["kids"] if k["t"] == "T" and k["name"].endswith(" label")]
            for label in labels:
                controls = [k for k in node["kids"] if k["name"] in (label, f"{label} icon", f"{label} dial")]
                self.assertTrue(controls, f"{kind}/{label}")
                self.assertTrue(all(c["abs"][0] >= 112.0 for c in controls), f"{kind}/{label}")

    def test_the_type_settings_rows_put_their_control_at_152_and_144_wide(self):
        node = popover("TypeSettings")
        for label in ("Number spacing", "Fractions", "Paragraph spacing", "Text direction", "Wrap style"):
            control = named(node, label)[0]
            self.assertEqual((control["abs"][0], control["abs"][2], control["abs"][3]), (152.0, 144.0, 24.0), label)
        self.assertEqual([k["name"] for k in node["kids"] if k["name"].startswith("Set ")], [f"Set {i}" for i in range(1, 9)])

    def test_a_popover_with_a_header_has_its_close_at_the_right_edge_and_the_hairline_across_it(self):
        for kind in ("EditStyleGrid", "Stroke", "TypeSettings"):
            node = popover(kind)
            w = node["w"]
            self.assertEqual(named(node, "Close")[0]["abs"][0:2], (w - 32.0, 8.0), kind)
            self.assertEqual(named(node, "Hairline")[0]["abs"][0:3], (0.0, 39.0, w), kind)
            self.assertEqual(named(node, "Hairline")[0]["set"], "Divider/Hairline", kind)

    def test_the_interaction_popover_removes_through_a_minus_button_at_the_title_rows_right(self):
        remove = named(popover("Interaction"), "Remove")[0]
        self.assertEqual((remove["set"], remove["props"], remove["abs"][1:]), ("Button/Icon", {"Icon": "Minus"}, (8.0, 24.0, 24.0)))

    def test_every_paint_is_a_semantic_name_and_the_only_literals_are_recorded_ones(self):
        for kind in SIZES:
            for node in ir.walk(popover(kind)):
                for key in ("fill", "stroke", "color"):
                    if node.get(key) is not None:
                        self.assertIsInstance(node[key], str, f"{kind}/{node['name']}: {key} is a literal")
                        self.assertRegex(node[key], r"^[A-Z][A-Z0-9_]*(\[\d+\])?$", f"{kind}/{node['name']}: {key} is not a theme constant name")
                for key in ("alpha", "sw", "radius", "gap", "pad"):
                    val = node.get(key)
                    for v in (val if isinstance(val, list) else [val]):
                        if isinstance(v, tuple):
                            self.assertEqual(v[0], "lit", f"{kind}/{node['name']}: {key}")
                            self.assertTrue(v[2], f"{kind}/{node['name']}: a literal {key} without its reason")

    def test_text_uses_known_styles_and_a_label_is_text_secondary(self):
        import text_styles_plan
        styles = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}
        for kind in SIZES:
            for node in ir.walk(popover(kind)):
                if node["t"] == "T":
                    self.assertIn(node["style"], styles, f"{kind}/{node['name']}")
        for kind in SETTINGS_ROWS:
            for label in (k for k in popover(kind)["kids"] if k["t"] == "T" and k["name"].endswith(" label")):
                self.assertEqual(label["color"], "TEXT_SECONDARY", f"{kind}/{label['name']}")

    def test_no_part_leaves_its_popover_unless_the_popover_clips(self):
        def walk(node, ox, oy, clipped):
            for k in node.get("kids", []):
                if k.get("abs"):
                    x, y, w, h = k["abs"]
                    if not clipped:
                        self.assertTrue(x >= 0 and y >= 0 and ox + x + w <= root_w + 0.01 and oy + y + h <= root_h + 0.01, f"{kind}/{k['name']} {k['abs']}")
                    walk(k, ox + x, oy + y, clipped or k.get("clip", False))
                else:
                    walk(k, ox, oy, clipped)
        for kind in SIZES:
            root = popover(kind)
            root_w, root_h = root["w"], root["h"]
            walk(root, 0, 0, root.get("clip", False))

    def test_the_erase_caption_runs_past_the_panel_which_clips_it(self):
        node = popover("Erase")
        self.assertTrue(node["clip"])
        self.assertGreater(named(node, "Mask")[0]["abs"][0] + named(node, "Mask")[0]["abs"][2], node["w"])

    def test_the_permissions_modal_nests_the_two_radio_cards_where_the_capture_names_them(self):
        cards = [k for k in popover("Permissions")["kids"] if k["t"] == "X" and k["set"] == "Radio/Card"]
        self.assertEqual([(c["variant"], c["abs"]) for c in cards], [("State=Default", (24.0, 54.0, 312.0, 68.0)), ("State=Selected", (24.0, 126.0, 312.0, 68.0))])

    def test_the_comment_thread_nests_the_close_resolve_and_disabled_reply_buttons(self):
        node = popover("CommentThread")
        self.assertEqual({k["name"]: (k["set"], k["variant"]) for k in node["kids"] if k["t"] == "X" and k["set"].startswith("Button")},
                         {"Resolve": ("Button/Icon", "Size=Xs, State=Default"), "Close": ("Button/Close", "Size=Xs, State=Default"), "Reply": ("Button/Primary", "Size=Xs, State=Disabled")})
        self.assertEqual(named(node, "Reply box")[0]["abs"], (12.0, 104.0, 276.0, 56.0))


if __name__ == "__main__":
    unittest.main()
