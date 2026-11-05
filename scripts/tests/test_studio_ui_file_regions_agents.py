"""Panel/AgentTerminal and Input/Composer of components/regions_sidepanels_agents.py and regions_sidepanels_composer.py (page `03 Studio`, row Agents), read from the IR the builders
return against the real Semantic plan. Lines are the v0.7.5 tag; the capture is `i-agents`, the region is the left panel (49, 32, 401, 868) and the composer box (61, 808, 376, 80),
so every rect here is relative to its own origin.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_agents
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio_ui_regions_fixtures as fx  # noqa: E402
from rest_ir import walk  # noqa: E402


class ComposerTest(unittest.TestCase):
    def setUp(self):
        self.box = fx.build("Input/Composer")

    def test_it_is_one_plain_record_in_the_agents_row_that_says_spec_i_supersedes_it(self):
        spec = fx.SPECS["Input/Composer"]
        self.assertEqual((spec["axes"], spec["row"], spec.get("uses", [])), ({}, "Agents", []))
        self.assertIn("Spec I", spec["description"])

    def test_the_box_is_376_by_80_bg_secondary_with_a_border_at_radius_frame(self):
        b = self.box
        self.assertEqual((b["w"], b["h"], b["fill"], b["stroke"], b["sw"], b["radius"]), (376, 80, "color/bg-secondary", "color/border", "size/border-width", "radius/frame"))

    def test_the_hint_and_the_control_row_sit_where_the_painter_puts_them(self):
        hint = fx.part(self.box, "Hint")
        self.assertEqual((hint["abs"], hint["text"], hint["style"], hint["color"]), ((12, 8, 352, 18), "Ask for changes", "Body / Regular", "color/text-tertiary"))
        rects = {n: fx.part(self.box, n)["abs"] for n in ("Attach", "Book", "PRD", "Skills", "Send")}
        self.assertEqual(rects, {"Attach": (8, 48, 24, 24), "Book": (36, 48, 24, 24), "PRD": (68, 48, 40, 24), "Skills": (112, 48, 46.75, 24), "Send": (340, 46, 28, 28)})

    def test_the_glyph_buttons_are_18_px_secondary_icons_and_the_send_circle_is_a_tertiary_disc_with_an_arrow(self):
        icons = {n: [(i["icon"], i["size"], i["color"]) for i in walk(fx.part(self.box, n)) if i["t"] == "I"] for n in ("Attach", "Book", "Send arrow")}
        self.assertEqual(icons, {"Attach": [("Plus", 18, "color/icon-secondary")], "Book": [("BookOpen", 18, "color/icon-secondary")], "Send arrow": [("ArrowUp", 18, "color/text-secondary")]})
        send = fx.part(self.box, "Send")
        self.assertEqual((send["t"], send["fill"], send["radius"][:2]), ("F", "color/bg-tertiary", ("lit", 14.0)), "agent_composer.rs:1158 `circle_filled`: a frame as round as a circle")
        self.assertIn("agent_composer.rs:1158", send["radius"][2])
        prd = fx.part(self.box, "PRD")
        self.assertEqual((prd["fill"], prd["stroke"], prd["radius"], fx.texts(prd)), ("color/bg-tertiary", "color/border", "radius/control", ["PRD"]))


class AgentsRegionTest(unittest.TestCase):
    def setUp(self):
        self.region = fx.build("Panel/AgentTerminal")

    def test_it_is_one_plain_record_in_the_regions_row_nesting_the_chip_and_the_composer_and_names_its_painter(self):
        spec = fx.SPECS["Panel/AgentTerminal"]
        self.assertEqual((spec["axes"], spec["row"], sorted(spec["uses"])), ({"State": ["Picker", "Chat", "Failure", "Approval"]}, "Regions", ["Card/Approval", "Input/Composer", "Tag/Chip"]))
        self.assertIn("v0.7.5", spec["description"])
        self.assertIn("Spec I", spec["description"])

    def test_the_frame_is_the_left_panel_401_by_868_on_bg(self):
        r = self.region
        self.assertEqual((r["w"], r["h"], r["fill"], r.get("clip")), (401, 868, "color/bg", True))

    def test_the_header_bar_has_the_sessions_button_the_runtime_button_and_the_skills_chip(self):
        rects = {n: fx.part(self.region, n)["abs"] for n in ("Bar", "Sessions", "Add session", "Skills")}
        self.assertEqual(rects, {"Bar": (0, 0, 401, 36), "Sessions": (10, 6, 76, 24), "Add session": (94, 6, 28, 24), "Skills": (130, 4, 46.75, 28)})
        self.assertEqual(fx.part(self.region, "Bar")["fill"], "color/bg-secondary")
        add = fx.part(self.region, "Add session")
        self.assertEqual((add["fill"], add["stroke"], add["radius"]), ("color/bg-selected", "color/border", "radius/control"))
        self.assertEqual([(i["icon"], i["size"]) for i in walk(add) if i["t"] == "I"], [("Plus", 11), ("ChevronDown", 8)])

    def test_the_strip_has_the_selection_chip_240_wide_the_mention_button_and_a_rule_on_row_59(self):
        chip = fx.part(self.region, "Selection chip")
        self.assertEqual((chip["t"], chip["abs"], chip["set"], chip["variant"], chip["w"]), ("X", (8, 36, 240, 24), "Tag/Chip", "Size=Sm, State=Default", 240))
        self.assertEqual(chip["props"], {"Label": "Page 1 · 1 selected · Frame 1"})
        mention = fx.part(self.region, "Mention")
        self.assertEqual((mention["abs"], mention["fill"], mention["stroke"], fx.texts(mention)), ((314.25, 38, 77.75, 20), "color/bg-secondary", "color/border", ["@ Mention"]))
        self.assertEqual(fx.part(self.region, "Strip rule")["abs"], (0, 59, 401, 1))

    def test_the_picker_card_is_the_menu_surface_with_seven_40_px_rows_44_apart(self):
        card = fx.part(self.region, "Picker")
        self.assertEqual((card["abs"], card["fill"], card["stroke"], card["radius"]), ((16, 76, 368, 386), "color/bg-menu", "color/border-menu", "radius/frame"))
        rows = [fx.part(card, f"CLI {n}") for n in range(1, 8)]
        self.assertEqual([r["abs"] for r in rows], [(16, 40 + 44 * i, 336, 40) for i in range(7)])
        self.assertEqual({(r["fill"], r["stroke"], r["radius"]) for r in rows}, {("color/bg-secondary", "color/border", "radius/control")})
        self.assertEqual([fx.texts(r)[0] for r in rows], ["Claude Code", "Codex", "Grok", "Antigravity", "Gemini CLI", "Other CLI…", "Shell"])

    def test_a_row_has_the_name_the_mono_version_and_the_state_pill_8_from_the_right(self):
        row = fx.part(self.region, "CLI 1")
        name, version, pill = fx.part(row, "Name"), fx.part(row, "Version"), fx.part(row, "Pill signed in")
        self.assertEqual((name["abs"][:2], name["style"], name["color"]), ((8, 11), "Body / Medium", "color/text"))
        self.assertEqual((version["abs"][:2], version["text"]), ((90.05, 11), "1.0.0"), "agent_picker.rs:376: name + SPACE_INLINE")
        self.assertEqual((version["style"], version["color"]), ("Mono / Value", "color/text-secondary"))
        self.assertEqual((pill["abs"], pill["fill"], pill["stroke"]), ((263.22, 11, 64.78, 18), "color/bg-success-primary", "color/badge-success-border"))
        self.assertEqual((fx.lit_of(pill["radius"]), fx.texts(pill)), (("lit", 9.0), ["Signed in"]))

    def test_an_unavailable_cli_is_dimmed_and_the_other_cli_row_carries_a_caption(self):
        grok = fx.part(self.region, "CLI 3")
        state = fx.part(grok, "Pill checking")
        self.assertEqual((fx.part(grok, "Name dim")["color"], fx.part(state, "Text")["color"], state["fill"]), ("color/text-disabled", "color/text-disabled", "color/bg-tertiary"))
        self.assertTrue(fx.part(grok, "Name")["hidden"] and not fx.part(grok, "Name dim").get("hidden"), "the default draws the dim name; the probed name is a hidden part")
        other = fx.part(self.region, "CLI 6")
        cap = fx.part(other, "Caption")
        self.assertEqual((cap["text"], cap["color"], cap["abs"][1], fx.part(other, "Name")["abs"][1]), ("Plain command: Studio tools not connected", "color/text-tertiary", 23, 3))

    def test_the_heading_the_five_prompts_the_hint_and_the_composer_follow_the_picker(self):
        heading = fx.part(self.region, "Heading")
        self.assertEqual((heading["abs"], fx.texts(heading), fx.part(heading, "Text")["style"]), ((0, 476, 401, 20), ["What's next?"], "Large / Semibold"))
        prompts = [fx.part(self.region, f"Prompt {n}") for n in range(1, 6)]
        self.assertEqual([p["abs"] for p in prompts], [(16, 510 + 44 * i, 368, 40) for i in range(5)])
        self.assertEqual(fx.texts(prompts[0]), ["Review this frame", "Review the selected frame for spacing, alignment and cont…"])
        title, body = fx.part(prompts[0], "Title"), fx.part(prompts[0], "Text")
        self.assertEqual((title["style"], body["style"], body["color"]), ("Body / Medium", "Caption / Regular", "color/text-secondary"))
        hint = fx.part(self.region, "Attach hint")
        self.assertEqual((hint["abs"], hint["text"]), ((16, 738, 368, 18), "Type @ to mention layers."))
        composer = fx.part(self.region, "Composer")
        self.assertEqual((composer["set"], composer["variant"], composer["abs"]), ("Input/Composer", "", (12, 776, 376, 80)))

    def test_no_kit_name_is_in_either_region_and_every_text_style_and_literal_is_defined(self):
        for tree in (self.region, fx.build("Input/Composer")):
            for t in fx.texts(tree):
                self.assertFalse(any(w in t.lower() for w in fx.KIT_WORDS), t)
            for k in walk(tree):
                if k["t"] == "T":
                    self.assertIn(k["style"], fx.STYLES)
            found = fx.lit_reasons(tree)
            self.assertTrue(found, "the region pins the pill radii and the composer its Send disc")
            for name, e in found:
                self.assertRegex(e[2], r"\w+\.rs:\d+", f"{name}: lit({e[1]})")


if __name__ == "__main__":
    unittest.main()
