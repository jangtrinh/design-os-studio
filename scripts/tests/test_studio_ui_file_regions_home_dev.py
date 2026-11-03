"""Panel/DevInspect (components/regions_home_dev.py) read from the IR the builder returns against the real Semantic plan. The rects are the 0.7.5 capture `g-dev-react` (`g-dev-react.layout.json`
named rects of a 1440 x 900 window): the region is the inspect body of the right panel, (1199, 113) 241 x 751, so every rect here is the capture's minus (1199, 113). The Rust is v0.7.5
`panels/inspect.rs` `show` / `body` (:313-640) and `dev/sections.rs` (:16-368). Structure only: pixels are compared in `evidence compare`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_home_dev
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


class DevInspectTest(unittest.TestCase):
    def setUp(self):
        self.spec = SPECS["Panel/DevInspect"]
        self.root = self.spec["build"]({"State": "Inspect"}, ENV)  # the first value is today's look

    def test_it_is_a_plain_region_record_clipped_to_the_inspect_body(self):
        """No registry variants; the body is `region` of `inspect::show` (:313-336): from y 113 to the scrollbar track's end (864), x 1199 (`c.x0`) to the window edge, `clipped` to it."""
        self.assertEqual(self.spec["axes"], {"State": ["Inspect", "Annotate", "Component", "List"]})
        self.assertEqual((self.root["w"], self.root["h"], self.root["fill"], self.root["clip"]), (241, 751, "color/bg", True))

    def test_every_named_rect_of_the_capture_is_the_part_of_the_same_name(self):
        """dev-inspect-link 1366, 127; dev-inspect-code 1388, 127; dev-inspect-more 1408, 127 (20 px); dev-complete 1215, 203, 101 x 28; dev-clear 1324, 203, 100 x 28; annot-add 1368, 248 (56 x
        24); link-new 1368, 373; dev-link-0 1215, 397 (181 x 36); dev-mcp-btn 1215, 442 (209 x 40); dev-mcp-prompt 1215, 550 (209 x 88); mcp-copy-prompt 1215, 646 (209 x 28); dev-code-connect
        1215, 681 (209 x 40); code-connect-terminal 1215, 783 (209 x 26); dev-add-redlines 1336, 826 (88 x 20)."""
        want = {"Link action": (167, 14, 20, 20), "Code action": (189, 14, 20, 20), "More action": (209, 14, 20, 20), "Mark completed": (16, 90, 101, 28), "Remove status": (125, 90, 100, 28),
                "Add annotation": (169, 135, 56, 24), "Add link": (169, 260, 56, 24), "Dev link": (16, 284, 181, 36), "MCP header": (16, 329, 209, 40), "MCP prompt": (16, 437, 209, 88),
                "Copy MCP link": (16, 533, 209, 28), "Code Connect header": (16, 568, 209, 40), "Send to agent": (16, 670, 209, 26), "Add redlines": (137, 713, 88, 20)}
        self.assertEqual({name: tuple(part(self.root, name)["abs"]) for name in want}, want)

    def test_the_section_hairlines_are_the_nested_divider_across_the_whole_width(self):
        """`section_title` (:225-231) and the MCP, Code Connect and Layer properties headers draw `hline(c.x0, c.x0 + c.w, y, BORDER)`: 241 wide at y 126, 251, 328, 567, 702."""
        rules = sorted(k["abs"] for k in walk(self.root) if k["t"] == "X" and k["set"] == "Divider/Hairline")
        self.assertEqual(rules, [(0, y, 241, 1) for y in (126, 251, 328, 567, 702)])
        self.assertEqual(self.spec["uses"], ["Button/Icon", "Divider/Hairline"])

    def test_the_status_pill_is_the_bound_ready_colour_at_a_paint_opacity_of_0_22_with_a_dot_and_the_complete_button_a_brand_pill(self):
        """`status_pill` (:31-40): a 20 px pill, the dot at left + 10 (`DEV_READY`, radius 3.5), the label at left + 19 in caption medium; `primary_button` (:42-48): `BG_BRAND` at `cr_pill(28)`, a
        `Body / Semibold` label. The pill is `DEV_READY.gamma_multiply(0.22)` (:36): the fill is bound to `color/dev-ready` and the paint's opacity is the literal 0.22
        (over BG it is (12, 46, 28))."""
        pill = part(self.root, "Status pill")
        self.assertEqual((pill["abs"], pill["fill"], pill["alpha"][:2], pill["radius"][:2]), ((16, 62, 106, 20), "color/dev-ready", ("lit", 0.22), ("lit", 10.0)))
        self.assertIn("gamma_multiply(0.22)", pill["alpha"][2])
        self.assertEqual((part(pill, "Dot")["fill"], part(pill, "Dot")["abs"][:2]), ("color/dev-ready", (6.5, 6.5)))
        button = part(self.root, "Mark completed")
        self.assertEqual((button["fill"], button["radius"][:2], part(button, "Label")["style"]), ("color/bg-brand", ("lit", 14.0), "Body / Semibold"))
        self.assertEqual((part(self.root, "Remove status")["fill"], part(self.root, "Remove status")["radius"]), ("color/bg-secondary", "radius/control"))

    def test_the_prompt_box_is_a_bordered_secondary_box_with_mono_text_in_text_and_info_colours(self):
        """`prompt_box` (code_view.rs:174-189): `BG_SECONDARY`, `BORDER_STRONG` 1 px inside, `RADIUS_CONTROL`, `INSET_CONTROL` inset, `font_mono` at `CODE_LINE_HEIGHT` 18: two lines in `TEXT`,
        then the link in `INFO`."""
        box = part(self.root, "MCP prompt")
        self.assertEqual((box["fill"], box["stroke"], box["radius"]), ("color/bg-secondary", "color/border-strong", "radius/control"))
        lead, uri = part(box, "Prompt lead"), part(box, "Prompt link")
        self.assertEqual((lead["style"], lead["color"], lead["abs"][:2]), ("Mono / Value", "color/text", (8, 8)))
        self.assertEqual((uri["style"], uri["color"], uri["abs"][:2]), ("Mono / Value", "color/info", (8, 44)))

    def test_the_description_is_four_single_lines_15_px_apart(self):
        """`painter.layout(.., font_body(), .., width)` gives egui's own 15 px rows (the capture's line centres are 15 apart), where the Body style's line is 18: each row is a text of its own."""
        rows = sorted((k["abs"][1], k["text"]) for k in walk(self.root) if k["name"].startswith("Connect text"))
        self.assertEqual([round(y - rows[0][0], 2) for y, _ in rows], [0, 15, 30, 45])
        self.assertEqual(rows[0][1], "Publish components to a library,")

    def test_the_neutral_frame_name_and_every_text_uses_an_existing_style_and_a_semantic_colour(self):
        names = {s["name"] for s in PLAN["semantic"]}
        for k in walk(self.root):
            if k["t"] == "T":
                self.assertIn(k["style"], STYLES)
                self.assertIn(k["color"], names)
        self.assertEqual(part(self.root, "Name")["text"], "Frame 1")
        self.assertIn("Frame-1", part(self.root, "Prompt link")["text"])


if __name__ == "__main__":
    unittest.main()
