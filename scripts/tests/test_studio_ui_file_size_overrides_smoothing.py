"""`CornerRadii` is also recorded when only `corner_smoothing` differs, as the editor does (`record_override`, designos-doc/src/components.rs:465:
`corner_radii != ... || corner_smoothing != ...`). No smoothing in the JSON is 0.0. Synthetic native documents (the editor-rule fixture).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_size_overrides_smoothing
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import size_overrides as so  # noqa: E402
from test_studio_ui_file_size_overrides_editor_rule import RING, RING_MAIN, doc, flags  # noqa: E402


def smoothed(instance, main):
    d = doc(ring=(16.0, 16.0))
    for nid, value in ((RING, instance), (RING_MAIN, main)):
        if value is None:
            d["nodes"][str(nid)].pop("corner_smoothing", None)
        else:
            d["nodes"][str(nid)]["corner_smoothing"] = value
    return d


def ring_problems(d):
    return [m for m in so.missing(d) if m["id"] == RING]


class Smoothing(unittest.TestCase):
    def test_a_different_smoothing_without_the_flag_is_reported(self):
        out = ring_problems(smoothed(0.6, None))
        self.assertEqual(len(out), 1)
        self.assertIn("CornerRadii", out[0]["problem"])

    def test_record_adds_the_flag(self):
        d = smoothed(0.6, 0.0)
        counts = so.record(d)
        self.assertEqual((flags(d, RING), counts["radii"]), (["Layout", "CornerRadii"], 1))
        self.assertEqual(ring_problems(d), [])

    def test_equal_smoothing_needs_none(self):
        self.assertEqual(ring_problems(smoothed(0.6, 0.6)), [])

    def test_no_smoothing_and_zero_are_the_same(self):
        self.assertEqual(ring_problems(smoothed(None, 0.0)), [])
        self.assertEqual(ring_problems(smoothed(0.0, None)), [])


if __name__ == "__main__":
    unittest.main()
