"""Focus/Ring radius arithmetic: pill hosts and Rust's rounding. `paint_ring` draws `(radius + SPACE_FACT).round()` (`kit/focus.rs:38`, f32::round:
halves away from zero), and the host's radius is what it draws: clamped to half its smaller side, so a pill (`cr_pill(h)` = `round(h / 2)`,
theme.rs:432-434) gives round(h / 2) + SPACE_FACT. Integer heights halve to .0 or .5, where floor, ceil and Python's round disagree
or coincide, so a fractional host radius is tested too.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_ring_rounding
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import studio_ui_gate_ring as ring  # noqa: E402
from test_studio_ui_gate_ring import RingHostBase  # noqa: E402


class RustRoundTest(unittest.TestCase):
    def test_halves_round_away_from_zero(self):
        self.assertEqual([ring.rust_round(x) for x in (12.5, 14.5, 10.5, 0.5, 6.3, 6.7)], [13.0, 15.0, 11.0, 1.0, 6.0, 7.0])


class PillHostTest(RingHostBase):
    def corner_verdict(self, host, height, ring_value, width=80.0):
        """The corner failures of a ring pinned to `ring_value` in a literal host of this radius and height."""
        self.put(host=host, size=(width, height), ring=ring_value)
        return self.corner_fails(self.bound(self.entries(ring_value)))

    def test_a_pill_of_an_odd_height_17_rounds_half_up(self):
        """cr_pill(17) = round(8.5) = 9, ring round(9 + 4) = 13: floor gives 12, Python's round of the half gives 12 too."""
        self.assertEqual(self.corner_verdict(9.0, 17.0, 13.0), [])
        for wrong in (12.0, 14.0):
            self.assertEqual(len(self.corner_verdict(9.0, 17.0, wrong)), 4, wrong)

    def test_a_pill_of_an_odd_height_21_rounds_half_up(self):
        """cr_pill(21) = round(10.5) = 11, ring 15: Python's round(14.5) is 14."""
        self.assertEqual(self.corner_verdict(11.0, 21.0, 15.0), [])
        self.assertEqual(len(self.corner_verdict(11.0, 21.0, 14.0)), 4)

    def test_a_pill_stored_as_a_huge_radius_is_clamped_to_half_its_height(self):
        """The engine draws min(radius, height / 2): 9999 on a 17 px host is 8.5, so the ring is round(8.5 + 4) = 13, not 255."""
        self.assertEqual(self.corner_verdict(9999.0, 17.0, 13.0), [])
        self.assertEqual(len(self.corner_verdict(9999.0, 17.0, 14.0)), 4)

    def test_a_pill_of_an_even_height_needs_no_rounding(self):
        self.assertEqual(self.corner_verdict(16.0, 32.0, 20.0), [])
        self.assertEqual(len(self.corner_verdict(16.0, 32.0, 19.0)), 4)

    def test_the_clamp_uses_the_smaller_side(self):
        """A 40 x 40 host with radius 100 draws a circle of radius 20: the ring is 24."""
        self.assertEqual(self.corner_verdict(100.0, 40.0, 24.0, width=40.0), [])
        self.assertEqual(len(self.corner_verdict(100.0, 40.0, 12.0, width=40.0)), 4)


class FractionalHostTest(RingHostBase):
    def corner_verdict(self, host, ring_value):
        self.put(host=host, size=(40.0, 40.0), ring=ring_value)
        return self.corner_fails(self.bound(self.entries(ring_value)))

    def test_a_fractional_host_radius_rounds_to_the_nearest_whole_pixel(self):
        """2.3 + 4 = 6.3: round gives 6, ceil gives 7. 2.7 + 4 = 6.7: round gives 7, floor gives 6."""
        self.assertEqual(self.corner_verdict(2.3, 6.0), [])
        self.assertEqual(len(self.corner_verdict(2.3, 7.0)), 4)
        self.assertEqual(self.corner_verdict(2.7, 7.0), [])
        self.assertEqual(len(self.corner_verdict(2.7, 6.0)), 4)

    def test_a_half_rounds_up_not_to_even(self):
        self.assertEqual(self.corner_verdict(10.5, 15.0), [])  # 14.5: Python's round() gives 14
        self.assertEqual(len(self.corner_verdict(10.5, 14.0)), 4)


if __name__ == "__main__":
    unittest.main()
