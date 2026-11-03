"""The dirty-tab dot of the top tab bar: a screen draws it exactly when its 0.7.5 capture does (the capture layout names a `tab-unsaved-dot-1` rect with a size), and no text the screen
gate replaces covers it, so the gate sees the dot. The set of states is read from the capture layouts, not from a list.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_unsaved_dot
"""
import importlib.util
import json
import math
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import screens  # noqa: E402
from evidence import screen_caps, screen_masks  # noqa: E402
from evidence.capture_dirs import captures_dir  # noqa: E402
from screens import gap_left_dev_data as left_dev  # noqa: E402
from screens import gap_table_citations as gtc  # noqa: E402

DOT = "tab-unsaved-dot-1"
CAPTURES = captures_dir(json.loads((HERE.parent / "studio_ui_file/evidence/crops.json").read_text(encoding="utf-8"))["captures"], None)
TAB_BAR = "Navigation/TopTabBar"
DOT_PATH = "Navigation/TopTabBar > project-tab-0 > Unsaved dot"  # the node `tab-unsaved-dot-1` names, below the screen (screen_geometry `PATH_SEP`)
# the tab bar mean each dot state measures with the dot drawn (`evidence screens`, 3 decimals): the 13 editor captures, the three Dev ones, and the Compare dialog's dimmed one.
# The cap of each is that mean rounded up to 0.01 with no margin, as the other measured pins of the gate data are.
MEASURED = {**dict.fromkeys(("fill-d9", "f-picker-gradient", "f-edit-style-color", "edit-style-effect", "edit-style-grid", "edit-style-text", "effect-noise", "effect-glass", "effect-texture",
                             "grid-styles", "ai-image-menu", "ai-image-erase", "ai-image-expand"), 0.3), **dict.fromkeys(("g-dev-react", "g-dev-annotate", "micro-dev-boxmodel"), 0.259),
            "g-dev-compare": 0.208}
W4D_GATE = HERE.parent / "studio_ui_file/screens/w4d_gate.py"
AI_IMAGE = ("ai-image-menu", "ai-image-erase", "ai-image-expand")


def dot_rect(state):
    """The capture's dirty-tab dot (x, y, w, h), or None when the layout does not name it or records it with no size (no dirty tab)."""
    rect = json.loads((CAPTURES / f"{state}.layout.json").read_text(encoding="utf-8"))["named"].get(DOT, {}).get("rect")
    return tuple(rect) if rect and rect[2] > 0 and rect[3] > 0 else None


def overlaps(a, b):
    return a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and b[1] < a[1] + a[3]


@unittest.skipUnless(CAPTURES.exists(), "the 0.7.5 captures are untracked")
class UnsavedDotTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.specs = screens.discover()
        cls.dots = {s["state"]: dot_rect(s["state"]) for s in cls.specs}
        cls.texts = screen_masks.TEXTS  # the screen gate's replaced texts: the framework's own rows and every gate data module's

    def test_the_captures_name_the_dot_at_one_rect_on_a_set_of_states(self):
        with_dot = {state: rect for state, rect in self.dots.items() if rect}
        self.assertEqual(set(with_dot.values()), {(301.0, 13.0, 6.0, 6.0)})
        self.assertTrue(set(left_dev.DEV) <= set(with_dot), "the four Dev mode captures show the dot")
        self.assertGreater(len(with_dot), len(left_dev.DEV))

    def test_a_screen_draws_the_dot_exactly_when_its_capture_does(self):
        wrong = []
        for spec in self.specs:
            shown = any((p.get("props") or {}).get("Show unsaved dot") for p in spec["place"] if p["component"] == TAB_BAR)  # the Present view places no tab bar
            if shown != (self.dots[spec["state"]] is not None):
                wrong.append(spec["state"])
        self.assertEqual(wrong, [], "states whose tab bar and capture disagree about the dot")

    def test_no_replaced_text_covers_the_dot_the_capture_shows(self):
        covering = [(state, what, tuple(rect)) for state, dot in self.dots.items() if dot for rect, what, _ in self.texts[state] if overlaps(tuple(rect), dot)]
        self.assertEqual(covering, [], "replaced texts that cover the capture's dot")

    def test_the_project_tab_title_mask_stops_where_the_captures_dot_starts(self):
        dot_x = {rect[0] for rect in self.dots.values() if rect}
        ends = {rect[0] + rect[2] for state, texts in self.texts.items() for rect, what, _ in texts if what == "project tab title"}
        self.assertEqual(len(dot_x), 1)
        self.assertEqual(ends, dot_x)

    def test_the_tab_bar_cap_of_a_dot_screen_is_its_measured_mean_rounded_up_with_a_cause(self):
        with_dot = {state for state, rect in self.dots.items() if rect}
        self.assertEqual(set(MEASURED), with_dot)
        for state in sorted(with_dot):
            cap = screen_caps.CAPS[state]["tab bar"]
            self.assertEqual(cap, math.ceil(round(MEASURED[state] * 100, 6)) / 100, state)
            self.assertLess(cap, screen_caps.DEFAULT_CAPS["tab bar"], f"{state}: the cap of a screen that draws the dot is under the editor default")
            self.assertTrue(screen_caps.CAUSES[state].get("tab bar"), f"{state}: a cap that is not the default has a cause")

    def test_the_ai_image_tab_bar_cap_is_read_from_the_measured_table_of_its_gate_data(self):
        spec = importlib.util.spec_from_file_location("w4d_gate_measured", W4D_GATE)
        gate = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(gate)
        for state in AI_IMAGE:
            self.assertEqual(gate._MEASURED[state]["tab bar"], MEASURED[state], state)
            self.assertEqual(screen_caps.CAPS[state]["tab bar"], gate._MEASURED[state]["tab bar"], state)

    def test_a_screen_names_the_dot_node_for_the_capture_dot_and_no_other(self):
        for spec in self.specs:
            alias = (spec.get("aliases") or {}).get("tab-unsaved-dot-1")
            self.assertEqual(alias, DOT_PATH if self.dots[spec["state"]] else None, spec["state"])

    def test_the_gap_row_of_the_dirty_tab_title_lists_the_captures_that_show_the_dot(self):
        row = next(e for e in gtc.GAP_TABLE if e["row"] == 24)
        self.assertEqual(set(row["screens"]), {state for state, rect in self.dots.items() if rect})
        self.assertEqual(set(gtc.UNSAVED_DOT) | set(left_dev.DEV), set(row["screens"]))


if __name__ == "__main__":
    unittest.main()
