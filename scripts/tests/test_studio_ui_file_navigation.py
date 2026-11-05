"""Tabs/Pill against the right panel's tab painter (panels/right.rs:112-128), synthetic env: no theme.rs, kit-states.json or MCP.

The kit `tab` (widgets/kit/tabs.rs:115-126) has no caller; the live painter is inline in `panels::right::show`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_navigation
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import rest_ir as ir  # noqa: E402

NUM = {"RIGHT_TABS_H": 24.0}


class FakeEnv:
    def c(self, const):
        return const

    def num(self, const):
        return NUM.get(const, 1.0)


SET = {s["name"]: s for s in components.discover()}["Tabs/Pill"]


def build(state):
    return SET["build"]({"State": state}, FakeEnv())


def label(node):
    return [k for k in ir.walk(node) if k["t"] == "T"][0]


class TabsPillTest(unittest.TestCase):
    def test_the_set_is_the_five_registry_states_in_the_navigation_row(self):
        self.assertEqual(SET["row"], "Navigation")
        self.assertEqual(len(components.matrix(SET["axes"])), 5)

    def test_geometry_is_right_tabs_h_by_label_plus_2_tab_pad_with_the_control_radius(self):
        for state in SET["axes"]["State"]:
            node = build(state)
            self.assertEqual((node["h"], node["sx"], node["sy"], node["radius"]), (24.0, "hug", "fixed", "RADIUS_CONTROL"), state)
            self.assertEqual(ir.pads(node), [None, "RIGHT_TAB_PAD_X", None, "RIGHT_TAB_PAD_X"], state)

    def test_selected_is_bg_secondary_text_and_semibold(self):
        node = build("Selected")
        self.assertEqual((node["fill"], label(node)["color"], label(node)["style"]), ("BG_SECONDARY", "TEXT", "Body / Semibold"))

    def test_hover_is_bg_hover_text_secondary_and_regular(self):
        node = build("Hover")
        self.assertEqual((node["fill"], label(node)["color"], label(node)["style"]), ("BG_HOVER", "TEXT_SECONDARY", "Body / Regular"))

    def test_default_has_no_fill(self):
        node = build("Default")
        self.assertIsNone(node.get("fill"))
        self.assertEqual((label(node)["color"], label(node)["style"]), ("TEXT_SECONDARY", "Body / Regular"))

    def test_focus_has_the_default_look_because_the_painter_has_no_ring(self):
        self.assertEqual(build("Focus"), build("Default"))
        self.assertNotIn("Focus/Ring", SET.get("uses", []))

    def test_disabled_is_the_default_at_half_alpha_on_every_paint(self):
        node, default = build("Disabled"), build("Default")
        self.assertEqual(label(node)["alpha"], 0.5)
        self.assertEqual({**node, "kids": None, "alpha": None}, {**default, "kids": None, "alpha": None})
        self.assertEqual(label(default).get("alpha", 1.0), 1.0)


if __name__ == "__main__":
    unittest.main()
