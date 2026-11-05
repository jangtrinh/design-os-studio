"""The icon-swap check is silent only when both twins lack geometry. A swapped glyph that draws nothing (empty `fill_geometry` and no paths) and a
main found for one twin only used to report SWAP-OK, and the controller's criterion for flipping SWAP_CHECK_FATAL is "0 mismatches under the
drawn-glyph check", so a swap that loses the whole glyph must not read as clean. Both stay reporting lines while SWAP_CHECK_FATAL is False.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_swap_silence
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import swap_check as swap  # noqa: E402
from test_studio_ui_file_swap_geometry import CHECK20, ID, MAINS, PLUS20, add_twin, doc_with_mains  # noqa: E402


def twins(control_draws=True, swapped_draws=True, control_main=ID["Plus"], swapped_main=ID["Check"]):
    """compare() of a Plus control and a Check swapped twin, each optionally drawing nothing, with an optional main id (999: no such main)."""
    doc = doc_with_mains(**MAINS)
    add_twin(doc, 10, control_main, 20.0, 20.0, PLUS20, PLUS20)
    add_twin(doc, 20, swapped_main, 20.0, 20.0, CHECK20, CHECK20)
    for base, draws in ((10, control_draws), (20, swapped_draws)):
        if not draws:
            glyph = doc["nodes"][str(base + 2)]
            glyph["fill_geometry"] = []
            glyph["kind"]["Vector"]["paths"] = []
    return swap.compare(swap.snapshot(doc, 10), swap.snapshot(doc, 20))


class DrawsNothingTest(unittest.TestCase):
    def test_a_correct_pair_is_still_clean(self):
        self.assertEqual(twins(), [])

    def test_a_swapped_glyph_that_draws_nothing_is_reported(self):
        out = twins(swapped_draws=False)
        self.assertEqual(len(out), 1, out)
        self.assertIn("swapped glyph draws nothing", out[0])
        self.assertIn("(2.5, 2.5)-(17.5, 17.5)", out[0])

    def test_a_control_glyph_that_draws_nothing_is_named_the_control(self):
        out = twins(control_draws=False)
        self.assertEqual(len(out), 1, out)
        self.assertIn("control glyph draws nothing", out[0])

    def test_silence_stays_when_both_twins_draw_nothing(self):
        self.assertEqual(twins(control_draws=False, swapped_draws=False), [])

    def test_the_report_names_it_while_pending_and_stops_the_build_when_fatal(self):
        results = [{"case": "Button/Tool Hover", "mismatches": twins(swapped_draws=False)}]
        self.assertTrue(swap.report(results, fatal=False)[0].startswith("SWAP-PENDING"))
        with self.assertRaises(SystemExit) as e:
            swap.report(results, fatal=True)
        self.assertIn("draws nothing", str(e.exception))


class MainFoundForOneTwinTest(unittest.TestCase):
    def test_a_swapped_twin_whose_icon_main_is_missing_is_reported(self):
        out = twins(swapped_main=999)
        self.assertEqual(len(out), 1, out)
        self.assertIn("swapped glyph's Icon main was not found", out[0])

    def test_a_control_twin_whose_icon_main_is_missing_is_named_the_control(self):
        out = twins(control_main=999)
        self.assertEqual(len(out), 1, out)
        self.assertIn("control glyph's Icon main was not found", out[0])

    def test_silence_stays_when_neither_twin_has_a_main(self):
        self.assertEqual(twins(control_main=999, swapped_main=999), [])


if __name__ == "__main__":
    unittest.main()
