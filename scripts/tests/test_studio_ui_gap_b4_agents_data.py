"""Panel/AgentTerminal State=Picker, Chat, Failure, Approval (components/regions_sidepanels_agents*.py) and the per-screen data modules
(screens/gap_dialogs_data.py, screens/gap_agents_data.py), read from the IR the builders return and from the plain dicts the data modules export.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b4_agents_data
"""
import json
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import studio_ui_regions_fixtures as fx  # noqa: E402
import components  # noqa: E402
from rest_ir import walk  # noqa: E402
from screens import gap_agents_data as agents, gap_dialogs_data as dialogs  # noqa: E402

PANEL = fx.SPECS["Panel/AgentTerminal"]
REGISTRY = json.loads((fx.REPO / "design/component-registry.json").read_text(encoding="utf-8"))
SCREENS = fx.REPO / "scripts/studio_ui_file/screens"


def panel(state):
    return PANEL["build"]({"State": state}, fx.ENV)


def axis_values(name):
    return components.registry_axes(REGISTRY)[name]


class PanelTest(unittest.TestCase):
    def test_picker_is_the_first_state_and_the_axes_are_the_registrys(self):
        components.check_specs([PANEL], REGISTRY)
        self.assertEqual(PANEL["axes"], {"State": ["Picker", "Chat", "Failure", "Approval"]})

    def test_the_probe_properties_default_to_the_capture_i_agents(self):
        defaults = {p["name"]: p["default"] for p in PANEL["props"]}
        self.assertEqual(len(defaults), 20)
        self.assertEqual(defaults, agents.probes("signed", "signed", "checking", "unknown", "checking"))

    def test_every_probed_row_holds_all_three_pills_and_the_dim_name_as_show_parts(self):
        p = panel("Picker")
        for n in (1, 2, 3, 4, 5):
            row = fx.part(p, f"CLI {n}")
            props = {k["name"]: k["visible_prop"] for k in walk(row) if k.get("visible_prop")}
            self.assertEqual(set(props.values()) - {f"Show probed {n}"}, {f"Show signed in {n}", f"Show unknown {n}", f"Show checking {n}"}, n)
            self.assertEqual({"Pill signed in", "Pill unknown", "Pill checking", "Name dim"} <= set(props), True, n)
        self.assertNotIn("visible_prop", fx.part(p, "CLI 6")["kids"][0], "the Other CLI and Shell rows have no probe")

    def test_chat_is_the_session_tab_and_the_pty_cursor(self):
        c = panel("Chat")
        self.assertEqual(fx.texts(fx.part(c, "Tab")), ["Claude Code #1"])
        self.assertEqual(fx.part(c, "Cursor")["abs"], (12.0, 68.0, 8.0, 15.0))
        self.assertEqual(fx.part(c, "Composer")["abs"], (12.0, 754.0, 376.0, 80.0))
        self.assertEqual(fx.texts(fx.part(c, "Notice")), ["Starting Claude Code… your message will be sent"])

    def test_the_header_is_one_row_so_the_controls_follow_the_tab(self):
        bar = fx.part(panel("Chat"), "Bar")
        self.assertEqual([k["name"] for k in bar["kids"]], ["Tab slot", "Sessions", "Add session", "Skills"])
        self.assertEqual((bar["dir"], bar["gap"], bar["h"]), ("H", "space/inline", 36.0))

    def test_failure_is_the_start_failure_card_over_the_picker_position_composer(self):
        f = panel("Failure")
        self.assertEqual(fx.part(f, "Failure card")["abs"], (12.0, 76.0, 376.0, 217.0))
        self.assertEqual(fx.texts(fx.part(f, "Failure card"))[:2], ["Antigravity failed to start", "agy rejected a launch flag"])
        self.assertEqual(fx.part(f, "Composer")["abs"][1], 776.0)
        self.assertNotIn("Notice", [k["name"] for k in walk(f)])

    def test_approval_nests_the_card_under_the_strip_and_shows_the_needs_approval_status(self):
        a = panel("Approval")
        card = fx.part(a, "Approval card")
        self.assertEqual((card["set"], card["sy"]), ("Card/Approval", "hug"))
        self.assertEqual(fx.part(a, "Approval")["abs"][:2], (4.0, 64.0))
        self.assertEqual(fx.part(a, "Selection chip")["w"], 183.75)
        self.assertIn("Needs approval", fx.texts(a))
        self.assertEqual(fx.texts(fx.part(a, "Notice")), ["Select an agent tab"])


class DataTest(unittest.TestCase):
    def test_the_two_approval_frames_differ_in_what_their_captures_show(self):
        cmd, queued = agents.AGENTS["i-agents-approval-command"], agents.AGENTS["agents-approval-queued"]
        self.assertNotEqual(cmd, queued)
        paths = {o["path"] for o in queued["overrides"]} - {o["path"] for o in cmd["overrides"]}
        self.assertEqual(paths, {"Approval/Approval card/Summary/Command", "Approval/Approval card/Header/Count"})
        self.assertIn({"path": "Approval/Approval card/Header/Count", "visible": True}, queued["overrides"])

    def test_each_picker_state_carries_the_probe_results_its_capture_shows(self):
        """Read off the captures' pills, row by row (Claude Code, Codex, Grok, Antigravity, Gemini CLI): `i-agents` Signed in, Signed in, Checking, Sign-in unknown, Checking;
        `i-agents-picker` and `i-terminal` Signed in, Signed in, then Sign-in unknown x3; `kit-agents` Checking, Checking, Sign-in unknown, Checking, Sign-in unknown."""
        s, u, c = agents.SIGNED, agents.UNKNOWN, agents.CHECKING
        want = {"i-agents": (s, s, c, u, c), "i-agents-picker": (s, s, u, u, u), "i-terminal": (s, s, u, u, u), "kit-agents": (c, c, u, c, u)}
        for state, results in want.items():
            self.assertEqual(agents.AGENTS[state]["props"], agents.probes(*results), state)

    def test_an_entry_carries_only_the_keys_it_sets_so_a_screen_can_wire_it_verbatim(self):
        for state, entry in agents.AGENTS.items():
            self.assertEqual([k for k, v in entry.items() if v is None], [], f"{state}: a None would reach apply_spec and the screen tests as `overrides: None`")
            self.assertLessEqual(set(entry), {"variant", "props", "overrides"}, state)
        self.assertEqual(set(agents.AGENTS["i-agents-chat"]), {"variant"})
        self.assertEqual(set(agents.AGENTS["i-agents"]), {"variant", "props"})
        self.assertEqual(set(agents.AGENTS["i-agents-approval-command"]), {"variant", "overrides"})

    def test_the_pty_states_draw_the_viewport_in_the_nearest_tone_and_the_strip_in_bg_primary(self):
        """The capture's viewport is TERM_BG_VIEWPORT 0E0E12 (14, 14, 18) from the strip's bottom to the panel's: HUD_POINTER_FILL (17, 18, 22) is 4 away, BG (10, 10, 10) 8 away;
        the strip above it is BG_PRIMARY. The start-failure painter fills its rect with BG (`agent_start_failure.rs:241`), so Failure keeps BG."""
        for state in ("Chat", "Approval"):
            p = panel(state)
            self.assertEqual(p["fill"], fx.ENV.c("HUD_POINTER_FILL"), state)
            self.assertEqual((fx.part(p, "Strip fill")["fill"], fx.part(p, "Strip fill")["abs"]), (fx.ENV.c("BG_PRIMARY"), (0.0, 36.0, 401.0, 24.0)), state)
        self.assertEqual(panel("Failure")["fill"], fx.ENV.c("BG"))
        self.assertNotIn("Strip fill", [k["name"] for k in walk(panel("Failure"))])

    def test_the_approval_strip_chip_is_neutral_text_elided_to_its_width(self):
        label = lambda state: fx.part(panel(state), "Selection chip")["props"]["Label"]
        self.assertEqual(label("Approval"), "Page 1 · 1 selected · …")
        self.assertEqual(label("Chat"), label("Failure"))
        for state in ("Picker", "Chat", "Failure", "Approval"):
            self.assertNotIn("ashboar", label(state).lower(), "no kit page name, truncated or not (G4 denies it)")

    def test_i_terminal_is_the_same_capture_as_i_agents_picker_and_no_other_pair_is_equal(self):
        a = agents.AGENTS
        self.assertEqual(a["i-terminal"], a["i-agents-picker"])
        states = [s for s in a if s != "i-terminal"]
        keys = [json.dumps(a[s], sort_keys=True) for s in states]
        self.assertEqual(len(set(keys)), len(keys), "every other agents state is its own drawing")

    def test_the_open_tab_frame_is_groks_card_with_show_prompt_and_its_own_tab(self):
        o = {x["path"]: x for x in agents.AGENTS["agents-approval-open-tab"]["overrides"]}
        self.assertEqual(o["Bar/Tab slot/Tab/Row/Title"]["text"], "Grok CLI #1")
        self.assertFalse(o["Approval/Approval card/Summary"]["visible"] or o["Approval/Approval card/Actions"]["visible"])
        self.assertTrue(o["Approval/Approval card/Prompt row"]["visible"])

    def test_every_entry_names_a_value_of_its_registry_axes(self):
        for table, record in ((dialogs.DIALOG, "Dialog/Modal"), (dialogs.PICKER, "Popover/ColorPicker"), (dialogs.PALETTE, "Dialog/CommandPalette"), (dialogs.BANNER, "Banner/Inline")):
            for state, e in table.items():
                for axis, value in e["variant"].items():
                    self.assertIn(value, axis_values(record)[axis], (state, axis))
        for state, e in agents.AGENTS.items():
            self.assertIn(e["variant"]["State"], axis_values("Panel/AgentTerminal")["State"], state)

    def test_the_dialogs_name_their_kind_and_pickers_place_fill_where_the_capture_has_it(self):
        self.assertEqual({s: e["variant"]["Kind"] for s, e in dialogs.DIALOG.items()},
                         {"k-libraries": "Libraries", "j-plugins": "Plugins", "plugin-install": "InstallPlugin", "g-dev-compare": "Compare"})
        self.assertEqual({s: e["variant"]["Kind"] for s, e in dialogs.PICKER.items()}, {"f-picker": "Fill", "picker-page": "Page", "f-picker-gradient": "Gradient"})
        self.assertEqual(dialogs.BANNER["j-version-history"]["xy"], (290, 32))

    def test_the_palette_hints_are_the_rust_strings(self):
        cmdk = (fx.REPO / "crates/designos-app/src/cmdk.rs").read_text(encoding="utf-8")
        menus = (fx.REPO / "crates/designos-app/src/panels/menus.rs").read_text(encoding="utf-8")
        self.assertIn(f'RichText::new("{dialogs.PALETTE["j-cmdk"]["extra"]["props"]["Placeholder"]}")', cmdk)
        self.assertIn(f'RichText::new("{dialogs.PALETTE["j-palette"]["extra"]["props"]["Placeholder"]}")', menus)

    def test_the_data_modules_are_plain_data_with_no_screens_and_no_gate_table(self):
        for m in (dialogs, agents):
            self.assertFalse(hasattr(m, "screens"))
            self.assertFalse(Path(m.__file__).name.endswith("_gate.py"))
        text = "\n".join((SCREENS / f).read_text(encoding="utf-8") for f in ("gap_dialogs_data.py", "gap_agents_data.py"))
        self.assertIsNone(re.search(r"^(CAPS|CAUSES|TEXTS|CONTENT|REGIONS|KEEP|SCRIMS|GEOMETRY|WORST_TILE)\b", text, re.M))


if __name__ == "__main__":
    unittest.main()
