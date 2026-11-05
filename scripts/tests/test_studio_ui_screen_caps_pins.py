"""A state's caps, worst-tile override and known geometry mismatches come from its gate data, each with a cause and measured numbers: no self-calibration. A cap is the editor-default cap of the
region of that name, or a pinned value with its cause; a worst-tile override has a cause; a named widget rect known to be off by more than 1 px is pinned with its numbers, and the gate exits 0 on the
pinned numbers and 1 on an unpinned mismatch, a moved number, a pin with no cause or a pin the screen no longer needs.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_caps_pins
"""
import sys
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
sys.path.insert(0, str(HERE.parent))
import screen_caps as caps  # noqa: E402
from test_studio_ui_screen_caps import MEASURED, scores  # noqa: E402

STATE = "x-home"
HOME = {"screen": 4.6, "tab bar": 0.4, "home sidebar": 9.1}
MEANS = {"screen": 4.0, "tab bar": 0.2, "home sidebar": 9.0}
OFF = {"key": "rail-File", "layout": [0.0, 88.0, 47.0, 56.0], "node": [0.0, 90.5, 47.0, 56.0]}
PIN = dict(OFF, cause="the rail's File button sits 2.5 px low in the region (RAIL_PAD)")


def home(means=None, tile=20.0, mismatched=()):
    s = scores(dict(MEANS, **(means or {})), tile=tile, state=STATE)
    s["regions"] = {n: v for n, v in s["regions"].items() if n in HOME}
    s["geometry"] = {"mismatched": [dict(m) for m in mismatched]}
    return s


def data(caps_table=None, causes=None, worst=None, pins=None):
    table = dict(HOME if caps_table is None else caps_table)
    why = {"home sidebar": "no editor-default region"} if causes is None else causes
    return [mock.patch.dict(caps.CAPS, {STATE: table}), mock.patch.dict(caps.CAUSES, {STATE: why}),
            mock.patch.dict(caps.WORST_TILES, {STATE: worst} if worst else {}), mock.patch.dict(caps.GEOMETRY_PINS, {STATE: pins} if pins else {})]


class Ctx:
    def __init__(self, **kw):
        self.patches = data(**kw)

    def __enter__(self):
        [p.start() for p in self.patches]

    def __exit__(self, *a):
        [p.stop() for p in self.patches]


class CauseTest(unittest.TestCase):
    def test_a_pinned_cap_with_its_cause_and_a_default_cap_without_one_pass(self):
        with Ctx():
            self.assertEqual(caps.judge(home()), [])

    def test_a_cap_that_is_not_the_editor_default_and_has_no_cause_fails_naming_both_numbers(self):
        with Ctx(causes={}):
            out = caps.judge(home())
        self.assertEqual(len(out), 1)
        self.assertIn("home sidebar", out[0])
        self.assertIn("9.1", out[0])
        self.assertIn("cause", out[0])

    def test_a_default_region_cap_changed_without_a_cause_fails_and_with_one_passes(self):
        table = dict(HOME, **{"tab bar": 0.5})
        with Ctx(caps_table=table):
            self.assertTrue(any("tab bar" in line and "0.4" in line for line in caps.judge(home())))
        with Ctx(caps_table=table, causes={"home sidebar": "x", "tab bar": "the home tab bar has no project tab: 0.45 measured"}):
            self.assertEqual(caps.judge(home()), [])

    def test_the_default_caps_are_exposed_for_the_rule(self):
        self.assertEqual(caps.DEFAULT_CAPS, caps.CAPS["d-editor-default"])
        self.assertEqual(caps.DEFAULT_CAPS["rail"], 2.6)


class WorstTileTest(unittest.TestCase):
    def test_a_state_without_an_override_keeps_the_global_34(self):
        with Ctx():
            self.assertEqual(caps.judge(home(tile=34.0)), [])
            self.assertEqual(len(caps.judge(home(tile=34.1))), 1)

    def test_an_override_with_a_cause_moves_the_limit_for_that_state_only(self):
        with Ctx(worst=48.0, causes={"home sidebar": "x", "worst tile": "the starter thumbnail: 46.9 measured"}):
            self.assertEqual(caps.judge(home(tile=48.0)), [])
            out = caps.judge(home(tile=48.1))
            self.assertEqual(len(out), 1)
            self.assertIn("48.0", out[0])
        self.assertEqual(len(caps.judge(scores(tile=40.0))), 1, "the editor state keeps its 34")

    def test_an_override_with_no_cause_fails(self):
        with Ctx(worst=48.0):
            out = caps.judge(home(tile=40.0))
        self.assertEqual(len(out), 1)
        self.assertIn("worst tile", out[0])
        self.assertIn("cause", out[0])


class GeometryPinTest(unittest.TestCase):
    def test_a_mismatch_nobody_pinned_fails_with_its_numbers(self):
        with Ctx():
            out = caps.judge(home(mismatched=[OFF]))
        self.assertEqual(len(out), 1)
        self.assertIn("rail-File", out[0])
        self.assertIn("90.5", out[0])
        self.assertIn("not pinned", out[0])

    def test_a_pinned_mismatch_at_its_pinned_numbers_passes_and_the_exit_code_is_0(self):
        with Ctx(pins=[PIN]):
            self.assertEqual(caps.judge(home(mismatched=[OFF])), [])
            self.assertEqual(caps.exit_code([home(mismatched=[OFF])]), 0)

    def test_a_pin_whose_numbers_moved_fails_on_either_rect(self):
        with Ctx(pins=[PIN]):
            for moved in (dict(OFF, node=[0.0, 91.5, 47.0, 56.0]), dict(OFF, layout=[0.0, 89.0, 47.0, 56.0])):
                out = caps.judge(home(mismatched=[moved]))
                self.assertEqual(len(out), 1, moved)
                self.assertIn("moved", out[0])
            self.assertEqual(caps.exit_code([home(mismatched=[dict(OFF, node=[0.0, 91.5, 47.0, 56.0])])]), 1)

    def test_a_pin_without_a_cause_fails(self):
        with Ctx(pins=[dict(PIN, cause="")]):
            self.assertTrue(any("cause" in line for line in caps.judge(home(mismatched=[OFF]))))

    def test_a_pin_the_screen_no_longer_needs_is_stale_and_fails(self):
        with Ctx(pins=[PIN]):
            out = caps.judge(home())
        self.assertEqual(len(out), 1)
        self.assertIn("stale", out[0])

    def test_one_pin_does_not_cover_another_widget(self):
        with Ctx(pins=[PIN]):
            out = caps.judge(home(mismatched=[OFF, dict(OFF, key="rail-Agents")]))
        self.assertEqual(len(out), 1)
        self.assertIn("rail-Agents", out[0])

    def test_the_editor_state_still_fails_on_any_mismatch(self):
        s = scores()
        s["geometry"]["mismatched"] = [{"key": "rail-File"}]
        self.assertEqual(len(caps.judge(s)), 1)
        self.assertEqual(caps.exit_code([s]), 1)
        self.assertEqual(caps.judge(scores({"rail": MEASURED["rail"]})), [])


if __name__ == "__main__":
    unittest.main()
