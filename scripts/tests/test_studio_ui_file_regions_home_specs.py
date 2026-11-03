"""Panel/SpecReader (components/regions_home_specs.py) read from the IR the builder returns against the real Semantic plan. No capture shows it: the reader is switched off in 0.7.5
(`shell.rs:1813` `let specs_reader_open = false;`), so every rect comes from v0.7.5 `specs.rs` `show_reader` (:2720-2900) and the plan marks the record UNVERIFIED-VISUAL.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_home_specs
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


class SpecReaderTest(unittest.TestCase):
    def setUp(self):
        self.spec = SPECS["Panel/SpecReader"]
        self.root = self.spec["build"]({}, ENV)

    def test_it_is_a_plain_region_record_with_a_document_slot_the_size_of_the_canvas(self):
        """No registry variants. The reader replaces the canvas (`show_reader(.., canvas_rect)`, shell.rs:1816): 909 x 868 in the default editor (d-editor-default regions.canvas). The editorial
        document of the active tab (`render_po_view_tab` and four more) differs per tab, so it is a native Slot."""
        self.assertEqual(self.spec["axes"], {})
        self.assertEqual([(p["name"], p["type"]) for p in self.spec["props"]], [("Document", "SLOT")])
        self.assertEqual((self.root["w"], self.root["h"], self.root["fill"]), (909, 868, "color/bg"))
        self.assertEqual((part(self.root, "Document")["slot"], part(self.root, "Document")["abs"]), ("Document", (0, 42, 909, 774)))

    def test_the_bar_is_42_high_with_the_four_buttons_right_to_left_6_apart(self):
        """:2731-2789: `bar_h` 42, `canvas_btn_w` 56 ending 10 from the right, then `theme_btn_w` 50, `copy_md_w` 62 and `share_btn_w` 48 each 6 further left, 24 high, centred on the bar."""
        self.assertEqual(part(self.root, "Bar")["abs"], (0, 0, 909, 42))
        want = {"Canvas": (843, 9, 56, 24), "Slate": (787, 9, 50, 24), "Copy .md": (719, 9, 62, 24), "Share": (665, 9, 48, 24)}
        self.assertEqual({n: tuple(part(self.root, n)["abs"]) for n in want}, want)
        for n in want:
            self.assertEqual((part(self.root, n)["fill"], part(self.root, n)["stroke"], part(self.root, n)["radius"][:2]), ("color/bg-secondary", "color/border", ("lit", 5.0)), n)

    def test_the_active_tab_has_a_two_pixel_info_underline_on_the_bars_last_rows(self):
        """:2805-2830: the active tab (PRD & Overview) in `font_caption_strong` and `TEXT`, an `INFO` (0x0D99FF) 2 px underline inset 6 at `bar.bottom() - 2`, the rest `font_caption_medium` in the
        subtle ink."""
        underline = part(self.root, "Tab underline")
        self.assertEqual((underline["fill"], underline["abs"][1], underline["abs"][3]), ("color/info", 40, 2))
        self.assertEqual((part(self.root, "Tab 1")["kids"][0]["style"], part(self.root, "Tab 2")["kids"][0]["style"]), ("Caption / Semibold", "Caption / Medium"))

    def test_the_agent_pill_floats_bottom_left_with_a_sparkle_and_an_info_send_disc(self):
        """:2851-2866: `ask_w` 460, 32 high at (20, bottom - 42), fully rounded, `doc_sheet` fill and a 1 px border; the sparkle at left + 16 (13 px) and the send disc (radius 10) at right - 16,
        both `INFO`."""
        pill = part(self.root, "Agent pill")
        self.assertEqual((pill["abs"], pill["fill"], pill["stroke"]), ((20, 826, 460, 32), "color/bg-secondary", "color/border"))
        self.assertEqual((part(pill, "Sparkle")["icon"], part(pill, "Sparkle")["color"]), ("Sparkle", "color/info"))
        self.assertEqual((part(pill, "Send disc")["fill"], part(pill, "Send disc")["abs"]), ("color/info", (434, 6, 20, 20)))

    def test_every_text_uses_an_existing_style_and_a_semantic_colour_and_no_kit_name(self):
        names = {s["name"] for s in PLAN["semantic"]}
        for k in walk(self.root):
            if k["t"] == "T":
                self.assertIn(k["style"], STYLES)
                self.assertIn(k["color"], names)
                self.assertTrue(all(w not in k["text"].lower() for w in ("untitled", "vsf", "pcp", "olivia")))


if __name__ == "__main__":
    unittest.main()
