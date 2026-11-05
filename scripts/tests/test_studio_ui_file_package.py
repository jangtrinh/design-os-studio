"""The `scripts/studio_ui_file` package: pure rules of the file generator (colour naming, contrast, SVG path scaling, and spec_kit).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_package
No MCP server and no Pillow: only the modules a CI path may import are loaded here.
"""
import re
import sys
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent
PKG = SCRIPTS / "studio_ui_file"
sys.path.insert(0, str(SCRIPTS))
sys.path.insert(0, str(PKG))


class NamingRuleTest(unittest.TestCase):
    def setUp(self):
        import colour_names
        self.names = colour_names

    def test_an_exact_palette_value_takes_its_palette_name(self):
        n = self.names.build_names([(10, 10, 10, 255), (0xFE, 0xFC, 0xE8, 255), (255, 255, 255, 255), (0xFA, 0xFA, 0xFA, 255)])
        self.assertEqual(n[(10, 10, 10, 255)], "neutral/950")
        self.assertEqual(n[(0xFE, 0xFC, 0xE8, 255)], "yellow/50")
        self.assertEqual(n[(255, 255, 255, 255)], "base/white")
        self.assertEqual(n[(0xFA, 0xFA, 0xFA, 255)], "neutral/50")  # zinc-50 is the same value; the first family wins

    def test_an_off_palette_colour_is_hue_slash_step_never_hex(self):
        name = self.names.build_names([(0xE3, 0x63, 0x23, 255)])[(0xE3, 0x63, 0x23, 255)]
        self.assertRegex(name, r"^orange/\d+$")
        self.assertEqual(int(name.split("/")[1]) % 25, 0)

    def test_alpha_adds_a_percent_suffix_and_a_taken_name_a_counter(self):
        n = self.names.build_names([(255, 255, 255, 26), (0, 0, 0, 128), (0xE3, 0x63, 0x23, 255), (0xE3, 0x64, 0x23, 255)])
        self.assertEqual(n[(255, 255, 255, 26)], "base/white-a10")
        self.assertEqual(n[(0, 0, 0, 128)], "base/black-a50")
        taken = sorted(n[c] for c in ((0xE3, 0x63, 0x23, 255), (0xE3, 0x64, 0x23, 255)))
        self.assertEqual(len(set(taken)), 2)
        self.assertTrue(taken[1].endswith("-2"))

    def test_an_off_palette_colour_never_takes_a_tailwind_name_that_holds_another_value(self):
        """Studio's own #FFA629, #7CC4F8, #C4EBA6, #A8E6DD and #FFE58F derive names that Tailwind gives to other values."""
        cols = [(0xFF, 0xA6, 0x29, 255), (0x7C, 0xC4, 0xF8, 255), (0xC4, 0xEB, 0xA6, 255), (0xA8, 0xE6, 0xDD, 255), (0xFF, 0xE5, 0x8F, 255)]
        n = self.names.build_names(cols)
        for c in cols:
            self.assertNotIn(n[c], self.names.TAILWIND_NAMES, n[c])
            self.assertRegex(n[c], r"-\d+$")

    def test_an_alpha_colour_takes_the_name_of_its_opaque_twin(self):
        n = self.names.build_names([(0xFF, 0xA6, 0x29, 255), (0xFF, 0xA6, 0x29, 128)])
        self.assertEqual(n[(0xFF, 0xA6, 0x29, 128)], n[(0xFF, 0xA6, 0x29, 255)] + "-a50")

    def test_the_naming_rule_documents_that_steps_can_run_out_of_order(self):
        self.assertIn("darker than", self.names.__doc__)

    def test_names_are_deterministic_whatever_the_input_order(self):
        cols = [(0x0D, 0x99, 0xFF, 255), (0x35, 0x82, 0xFF, 255), (0x0C, 0x8C, 0xE9, 255), (0x35, 0x82, 0xFF, 51)]
        self.assertEqual(self.names.build_names(cols), self.names.build_names(list(reversed(cols))))

    def test_the_vendored_palette_names_its_source_and_version(self):
        import json
        header = json.loads((PKG / "tailwind-v3-palette.json").read_text())["_source"]
        self.assertEqual(header["version"], "v3.4.17")
        self.assertTrue(header["url"].startswith("https://") and "tailwindcss" in header["url"])


class ContrastTest(unittest.TestCase):
    def test_wcag_ratios_match_the_published_values(self):
        import specimens_contrast as c
        grey, ink, card = {"r": 0x73, "g": 0x73, "b": 0x73}, {"r": 10, "g": 10, "b": 10}, {"r": 23, "g": 23, "b": 23}
        self.assertAlmostEqual(c.ratio(grey, ink), 4.18, places=2)
        self.assertAlmostEqual(c.ratio(grey, card), 3.78, places=2)
        self.assertAlmostEqual(c.ratio({"r": 255, "g": 255, "b": 255}, {"r": 0, "g": 0, "b": 0}), 21.0, places=6)


class PathAndSpecTest(unittest.TestCase):
    def test_a_path_scales_by_16_over_256_and_keeps_arc_flags(self):
        import icons_rest
        self.assertEqual(icons_rest.scale_path("M16 32L32 16a8,8,0,0,1-8,8z"), "M1 2 L2 1 a0.5 0.5 0 0 1 -0.5 0.5 z")

    def test_a_malformed_path_fails_loudly(self):
        import icons_rest
        for bad in ("M1", "M1 2 Q", "X1 2"):
            with self.assertRaises(ValueError):
                icons_rest.scale_path(bad)

    def test_padding_entries_are_a_variable_zero_or_a_recorded_literal(self):
        import spec_kit
        box = spec_kit.box("B", direction="row", pad=[None, "inset/control", None, 4])
        self.assertEqual(box["layout"]["padding"], [0, "var:Semantic/inset/control", 0, 4])
        self.assertEqual(spec_kit.box("B", fill=None)["fill"], "none")  # never the default white fill

    def test_inset_samples_sit_in_the_radius_of_the_element_that_owns_the_inset(self):
        """P4: a control and a button are RADIUS_CONTROL 8; a frame, popover, panel and modal are RADIUS_FRAME 16."""
        import specimens_scales as s
        owner = s.INSET_OUTER
        self.assertEqual([owner[k] for k in ("segment", "control", "button")], ["radius/control"] * 3)
        self.assertEqual([owner[k] for k in ("frame", "panel", "popover", "modal")], ["radius/frame"] * 4)
        plan = {"semantic": [{"group": "radius", "name": f"radius/{n}", "value": float(v)}
                             for n, v in (("mark", 2), ("segment", 6), ("control", 8), ("xl", 12), ("frame", 16))]}
        self.assertEqual(s.concentric_radius(plan, owner["segment"], 2), "radius/segment")
        self.assertEqual(s.concentric_radius(plan, owner["frame"], 8), "radius/control")
        self.assertIsNone(s.concentric_radius(plan, owner["control"], 8))  # 8 - 8 = 0: the tile is square
        self.assertIsNone(s.concentric_radius(plan, owner["button"], 12))

    def test_every_generated_semantic_name_matches_the_gate_rule(self):
        """A synthetic theme (never the real one, so a theme.rs edit cannot turn this red) covering each naming branch."""
        import studio_ui_theme_policy as policy
        import studio_ui_theme_reader as reader
        import variables_plan
        src = ("pub const BG_SECONDARY: Color32 = rgb(0x171717);\npub const AGENT_COLORS_X: [Color32; 2] = [BG_SECONDARY, rgba(0xFFFFFF, 0x1A)];\n"
               "pub const SPACE_FACT: f32 = 4.0;\npub const INSET_PANEL: f32 = 16.0;\npub const CONTROL_XS: f32 = 32.0;\n"
               "pub const RADIUS_FRAME: u8 = 16;\npub const FONT_SIZE_CAPTION: f32 = 12.0;\npub const LINE_BODY: f32 = 18.0;\n"
               "pub const UI_FONT_FAMILY: &str = \"Inter\";\npub const UI_FONT_WEIGHT: f32 = 400.0;\npub const RAIL_FIRST_Y: f32 = 56.0;\n"
               "pub const RAIL_WIDTH: f32 = 48.0;\n")
        rows = reader.drawable_rows(reader.flatten(reader.read_theme_constants(src)))
        plan = variables_plan.plan_from_rows(rows, {"BG_SECONDARY": "colors.surface"})
        rule = re.compile(policy.SEMANTIC_NAME)
        names = [s["name"] for s in plan["semantic"]]
        self.assertEqual([n for n in names if not rule.fullmatch(n)], [])
        self.assertEqual(len(names), len(set(names)))
        self.assertIn("color/agent-colors-x-0", names)  # array elements count from 0, like their code syntax
        self.assertEqual(next(s for s in plan["semantic"] if s["name"] == "color/bg-secondary")["description"], "token colors.surface")


if __name__ == "__main__":
    unittest.main()
