"""Code/Block of components/data_views.py (row `Data & Code` of page `03 Studio`), read from the IR the builder returns against the real Semantic plan, with the Rust each
value comes from named in the module. Structure only: no 0.7.5 capture shows the block's pixels (the dev captures put it below the 900 px fold), so it is UNVERIFIED-VISUAL.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_code_block
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
import components.data_views  # noqa: E402,F401  (the module under test)
from components.env import Env  # noqa: E402
from rest_ir import pads, walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPECS = {s["name"]: s for s in components.discover() if s["module"] == "data_views"}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}
SYNTAX = {f"color/syntax-{k}" for k in ("plain", "property", "keyword", "string", "number", "type", "function", "comment", "punct")}


def part(node, name):
    found = [k for k in walk(node) if k["name"] == name]
    assert len(found) == 1, (name, len(found))
    return found[0]


class CodeBlockTest(unittest.TestCase):
    """`code_block` (dev/code_view.rs:124-170), the one caller `dev/sections.rs:473`."""

    def setUp(self):
        self.spec = SPECS["Code/Block"]
        self.block = self.spec["build"]({}, ENV)

    def test_it_is_a_plain_record_on_the_data_and_code_row_with_a_code_slot(self):
        self.assertEqual((self.spec["axes"], self.spec["row"], self.spec.get("uses", [])), ({}, "Data & Code", []))
        self.assertEqual([(p["name"], p["type"]) for p in self.spec["props"]], [("Code", "SLOT")])
        view = part(self.block, "Code")
        self.assertEqual((view["slot"], view["clip"], view["sx"]), ("Code", True, "fill"))

    def test_the_box_is_the_panels_209_px_in_bg_secondary_at_radius_control_with_no_stroke(self):
        """`ui.painter().rect_filled(r, cr(RADIUS_CONTROL), BG_SECONDARY)` (:143); the box is `c.w - 2 * PAD` wide (the Inspect panel's content, 209 in the capture layout)."""
        self.assertEqual((self.block["w"], self.block["fill"], self.block["radius"]), (209, "color/bg-secondary", "radius/control"))
        self.assertNotIn("stroke", self.block)

    def test_the_inset_is_inset_control_on_top_and_both_sides(self):
        self.assertEqual(pads(self.block)[:2] + [pads(self.block)[3]], ["inset/control"] * 3)

    def test_the_lines_are_18_px_rows_of_the_monospace_style_in_the_syntax_colours(self):
        view = part(self.block, "Code")
        self.assertEqual({k["h"] for k in view["kids"]}, {18})
        texts = [k for k in walk(view) if k["t"] == "T"]
        self.assertEqual({t["style"] for t in texts}, {"Mono / Value"})
        self.assertLessEqual({t["color"] for t in texts}, SYNTAX)
        self.assertEqual(ENV.num("CODE_LINE_HEIGHT"), 18)
        self.assertGreater(len(view["kids"]), 1)

    def test_a_declaration_is_coloured_as_the_tokenizer_colours_it(self):
        """highlight.rs `line`: `border-radius: 12px;` is Property, Punct, Number, Punct; a leading space belongs to the next span."""
        line = part(self.block, "Line 6")
        self.assertEqual([(t["text"], t["color"]) for t in line["kids"]],
                         [("  border-radius", "color/syntax-property"), (":", "color/syntax-punct"), (" 12px", "color/syntax-number"), (";", "color/syntax-punct")])

    def test_the_selector_and_the_call_follow_the_tokenizer(self):
        self.assertEqual([(t["text"], t["color"]) for t in part(self.block, "Line 1")["kids"]], [(".", "color/syntax-punct"), ("card", "color/syntax-plain"), (" {", "color/syntax-punct")])
        call = {t["text"]: t["color"] for t in part(self.block, "Line 7")["kids"]}
        self.assertEqual(call[" rgba"], "color/syntax-function")

    def test_the_scroll_thumb_follows_the_box_the_row_gap_and_the_strip_under_the_code(self):
        """The thumb sits 8 above the box's bottom, 4 high, `RADIUS_MARK`, `TEXT_TERTIARY` (:163-167): 4 under the last row, the thumb, 4 below it, so the box is
        rows * 18 + 2 * INSET_CONTROL + SPACE_FACT (a hidden part's strip)."""
        thumb = part(self.block, "Scroll thumb")
        self.assertEqual((thumb["h"], thumb["fill"], thumb["radius"]), (4, "color/text-tertiary", "radius/mark"))
        self.assertEqual((self.block["gap"], pads(self.block)[2]), ("space/fact", "space/fact"))
        self.assertEqual([k["name"] for k in self.block["kids"]], ["Code", "Scroll thumb"])

    def test_the_thumb_is_as_wide_as_the_share_of_the_code_the_box_shows_and_never_under_a_pointer_target(self):
        """`SideView::new`: view_w ** 2 / content_w clamped to TARGET_MIN..view_w, content_w = widest line * the monospace advance (0.6 em of FONT_SIZE_BODY)."""
        view_w = self.block["w"] - 2 * ENV.num("INSET_CONTROL")
        widest = max(sum(len(t["text"]) for t in line["kids"]) for line in part(self.block, "Code")["kids"])
        want = view_w * view_w / (widest * 0.6 * ENV.num("FONT_SIZE_BODY"))
        thumb = part(self.block, "Scroll thumb")
        self.assertAlmostEqual(thumb["w"], round(want, 2), places=2)
        self.assertGreaterEqual(thumb["w"], ENV.num("TARGET_MIN"))
        self.assertLessEqual(thumb["w"], view_w)

    def test_every_style_exists(self):
        self.assertLessEqual({k["style"] for k in walk(self.block) if k["t"] == "T"}, STYLES)


if __name__ == "__main__":
    unittest.main()
