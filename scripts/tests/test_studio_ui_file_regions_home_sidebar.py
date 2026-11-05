"""Panel/HomeSidebar (components/regions_home_sidebar.py) read from the IR the builder returns against the real Semantic plan. The rects are the 0.7.5 capture `h-home`
(`h-home.layout.json` named rects, a 1440 x 900 window whose Home area starts at y 32, so every rect here is the capture's minus (0, 32)); the Rust is v0.7.5 `home/mod.rs`
`mod sidebar` (:890-1168). Structure only: pixels are compared in `evidence compare`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_home_sidebar
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import icon_lib  # noqa: E402
import text_styles_plan  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
from components.env import Env  # noqa: E402
from rest_ir import walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPECS = {s["name"]: s for s in components.discover()}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def part(node, name):
    found = [k for k in walk(node) if k["name"] == name]
    assert len(found) == 1, (name, len(found))
    return found[0]


class HomeSidebarTest(unittest.TestCase):
    def setUp(self):
        self.spec = SPECS["Panel/HomeSidebar"]
        self.root = self.spec["build"]({}, ENV)

    def test_it_is_a_plain_region_record_that_nests_the_components_it_reuses(self):
        """The registry gives the region no variants, so it is one component (axes {}), not a set; `composes=Input/Search,List/Row,Badge/StatusCount` plus the icon button and the hairline."""
        self.assertEqual(self.spec["axes"], {})
        self.assertEqual(sorted(self.spec["uses"]), ["Badge/StatusCount", "Button/Icon", "Divider/Hairline", "Input/Search", "List/Row"])
        self.assertEqual({k["set"] for k in walk(self.root) if k["t"] == "X"}, set(self.spec["uses"]))

    def test_the_region_is_the_home_sidebar_264_wide_and_868_high_on_the_page_colour(self):
        """`HOME_SIDEBAR_W` 264 by the window less its 32 px tab bar (h-home.layout.json regions.home: 0, 32, 1440, 868); `p.rect_filled(rect, 0.0, BG)` (:905)."""
        self.assertEqual((self.root["w"], self.root["h"], self.root["fill"]), (264, 868, "color/bg"))

    def test_every_named_widget_rect_of_the_capture_is_the_nested_instance(self):
        """h-home.layout.json `named`, minus the area's origin (0, 32): the rows are 247 x 36 at x 8 (`w - 16`, `CONTROL_SM`), the search 239 x 36 at x 12."""
        want = {"Recents": (8, 98, 247, 36), "Templates": (8, 136, 247, 36), "Drafts": (8, 214, 247, 36), "All folders": (8, 251, 247, 36), "Resources": (8, 288, 247, 36),
                "Project": (8, 329, 247, 36), "Starred": (8, 726, 247, 36), "Trash": (8, 823, 247, 36), "Search": (12, 52, 239, 36), "Notifications": (229, 16, 24, 24),
                "New project": (227, 188, 24, 24)}
        got = {name: tuple(part(self.root, name)["abs"]) for name in want}
        self.assertEqual(got, want)

    def test_the_rows_are_the_small_list_row_with_the_capture_icons_and_labels(self):
        rows = [("Recents", "Selected", "Recents", "Clock"), ("Templates", "Default", "Templates & Specs", "Specs"), ("Drafts", "Default", "Drafts", "File"),
                ("All folders", "Default", "All folders", "GridView"), ("Resources", "Default", "Resources & Libraries", "Assets"), ("Project", "Default", "Getting started", "Frame"),
                ("Starred", "Default", "Starred", "Star"), ("Trash", "Default", "Trash", "Trash")]
        for name, state, label, icon in rows:
            row = part(self.root, name)
            self.assertEqual((row["set"], row["variant"], row["props"]), ("List/Row", f"Size=Sm, State={state}", {"Label": label, "Icon": icon}), name)

    def test_the_icon_buttons_are_the_extra_small_button_resized_to_the_24_px_the_painter_gives_them(self):
        for name, icon in (("Notifications", "Eye"), ("New project", "Plus")):
            button = part(self.root, name)
            self.assertEqual((button["set"], button["variant"], button["props"], button["abs"][2:]), ("Button/Icon", "Size=Xs, State=Default", {"Icon": icon}, (24, 24)), name)

    def test_the_version_pill_is_the_small_neutral_count_badge_at_the_end_of_the_workspace_row(self):
        """`kit::count_badge(.., Tone::Neutral)` (:928), `ver_w` = text + 2 * SPACE_FACT, ends SPACE_FACT before the row's right edge (8 + 219 - 4 = 223)."""
        badge = part(self.root, "Version")
        self.assertEqual((badge["set"], badge["variant"], badge["props"]), ("Badge/StatusCount", "Size=Sm, Tone=Neutral, State=Default", {"Label": "v0.7.5"}))
        self.assertEqual((badge["abs"][0] + badge["abs"][2], badge["abs"][1] + badge["abs"][3] / 2), (223, 28))

    def test_three_hairlines_and_the_right_edge_are_border_coloured(self):
        rules = {k["name"]: k["abs"] for k in walk(self.root) if k["t"] == "X" and k["set"] == "Divider/Hairline"}
        self.assertEqual(rules, {"Rule after navigation": (12, 180, 239, 1), "Rule above starred": (12, 720, 239, 1), "Rule above trash": (0, 814, 263, 1)})
        edge = part(self.root, "Edge")
        self.assertEqual((edge["abs"], edge["fill"]), ((263, 0, 1, 868), "color/border"))

    def test_the_workspace_avatar_and_the_project_disc_are_local_frames_with_the_production_paints(self):
        avatar, disc = part(self.root, "Workspace avatar"), part(self.root, "Project badge")
        self.assertEqual((avatar["abs"], avatar["fill"], avatar["stroke"], avatar["radius"]), ((12, 18, 20, 20), "color/bg-tertiary", "color/border-strong", "radius/control"))
        self.assertEqual([(k["icon"], k["size"], k["color"]) for k in avatar["kids"]], [("Folder", 12, "color/icon-brand-primary")])
        self.assertEqual((disc["abs"], disc["fill"], disc["radius"][:2]), ((13, 193, 14, 14), "color/tile-design-bg", ("lit", 7.0)))
        self.assertEqual([(k["text"], k["style"], k["color"]) for k in disc["kids"]], [("P", "Text xs / Semibold", "color/tile-design-glyph")])

    def test_every_text_uses_an_existing_style_and_a_semantic_colour(self):
        names = {s["name"] for s in PLAN["semantic"]}
        for k in walk(self.root):
            if k["t"] == "T":
                self.assertIn(k["style"], STYLES)
                self.assertIn(k["color"], names)

    def test_the_starred_chevron_is_a_quaternary_glyph_at_the_rows_right_end(self):
        """:1111-1116 `Icon::ChevronDown`, 11 px, `ICON_QUATERNARY`, centred at `star_r.right() - 12` = 8 + 247 - 12 = 243 (starred_expanded starts true)."""
        chevron = part(self.root, "Starred chevron")
        self.assertEqual((chevron["icon"], chevron["size"], chevron["color"], chevron["abs"][0] + 5.5, chevron["abs"][1] + 5.5), ("ChevronDown", 11, "color/icon-quaternary", 243, 744))


if __name__ == "__main__":
    unittest.main()
