"""Gradient paint in the IR: `F(..., fills=[grad(stops, start, end), ...])` instead of `fill`. Each stop is a literal colour with the reason no Semantic variable holds it: the REST node carries a
`GRADIENT_LINEAR` paint per layer (handles by Figma's convention, stops in 0..1), the build records one allow-list entry per stop (`fills/<layer>/stops/<stop>`, the rgba as 0..255 integers: the
gate's per-stop key and `_rgba`), and no bind intent is made for a fill that no variable owns. Synthetic plan only (no MCP).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_gradient_paint
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import rest_ir as ir  # noqa: E402
from rest_lower import Lowerer  # noqa: E402
from test_studio_ui_file_components import FakeIcons, PLAN, STYLES  # noqa: E402

WHITE, RED, CLEAR, BLACK = (255, 255, 255, 255), (255, 0, 0, 255), (0, 0, 0, 0), (0, 0, 0, 255)
LTR = [(0, WHITE, "start: white"), (1, RED, "end: the hue")]


def lower(node, kind="F"):
    low = Lowerer(PLAN, STYLES, FakeIcons())
    return low, low.lower(node, 0, 0, ptype="COMPONENT")


def box(**kw):
    return ir.F("Strip", w=100, h=20, sx="fixed", sy="fixed", **kw)


class GradConstructorTest(unittest.TestCase):
    def test_a_gradient_needs_two_stops_in_rising_order_inside_0_to_1(self):
        for bad in ([(0, WHITE, "r")], [(1, WHITE, "r"), (0, RED, "r")], [(-0.1, WHITE, "r"), (1, RED, "r")], [(0, WHITE, "r"), (1.5, RED, "r")]):
            with self.assertRaises(ValueError, msg=bad):
                ir.grad(bad)

    def test_a_stop_is_four_integer_channels_in_0_to_255_with_a_reason(self):
        for colour in ((255, 255, 255), (1.0, 0, 0, 1), (256, 0, 0, 255), (-1, 0, 0, 255), (True, 0, 0, 255), "red"):
            with self.assertRaises(ValueError, msg=colour):
                ir.grad([(0, colour, "r"), (1, RED, "r")])
        with self.assertRaises(ValueError):
            ir.grad([(0, WHITE, ""), (1, RED, "r")])
        with self.assertRaises(ValueError):
            ir.grad([(0, WHITE, None), (1, RED, "r")])

    def test_the_two_points_differ(self):
        with self.assertRaises(ValueError):
            ir.grad(LTR, start=(0.5, 0.5), end=(0.5, 0.5))

    def test_fill_and_fills_together_are_refused(self):
        with self.assertRaises(ValueError):
            ir.F("Strip", fill="color/bg", fills=[ir.grad(LTR)])


class LoweringTest(unittest.TestCase):
    def test_a_frame_with_one_gradient_layer_is_a_gradient_linear_paint(self):
        _, rest = lower(box(fills=[ir.grad(LTR)]))
        paint = rest["fills"][0]
        self.assertEqual((len(rest["fills"]), paint["type"]), (1, "GRADIENT_LINEAR"))
        self.assertEqual([(s["position"], s["color"]) for s in paint["gradientStops"]],
                         [(0, {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}), (1, {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0})])

    def test_left_to_right_has_figmas_three_handles(self):
        handles = lower(box(fills=[ir.grad(LTR)]))[1]["fills"][0]["gradientHandlePositions"]
        self.assertEqual(handles, [{"x": 0.0, "y": 0.5}, {"x": 1.0, "y": 0.5}, {"x": 0.0, "y": 1.0}])

    def test_top_to_bottom_runs_from_the_top_edge_to_the_bottom_edge(self):
        handles = lower(box(fills=[ir.grad(LTR, start=(0.5, 0.0), end=(0.5, 1.0))]))[1]["fills"][0]["gradientHandlePositions"]
        self.assertEqual((handles[0], handles[1]), ({"x": 0.5, "y": 0.0}, {"x": 0.5, "y": 1.0}))

    def test_an_alpha_stop_keeps_its_alpha(self):
        stops = lower(box(fills=[ir.grad([(0, CLEAR, "r"), (1, BLACK, "r")])]))[1]["fills"][0]["gradientStops"]
        self.assertEqual([s["color"]["a"] for s in stops], [0.0, 1.0])

    def test_two_layers_are_two_paints_bottom_first(self):
        _, rest = lower(box(fills=[ir.grad(LTR), ir.grad([(0, CLEAR, "r"), (1, BLACK, "r")], start=(0.5, 0.0), end=(0.5, 1.0))]))
        self.assertEqual([p["type"] for p in rest["fills"]], ["GRADIENT_LINEAR", "GRADIENT_LINEAR"])
        self.assertEqual(rest["fills"][1]["gradientStops"][1]["color"], {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0})

    def test_each_stop_is_one_allow_list_entry_with_its_reason_and_the_rgba_it_pins(self):
        low, rest = lower(box(fills=[ir.grad(LTR), ir.grad([(0, CLEAR, "clear reason"), (1, BLACK, "black reason")])]))
        entries = [(a["property"], a["value"], a["reason"]) for a in low.allow]
        self.assertEqual(entries, [("fills/0/stops/0", {"r": 255, "g": 255, "b": 255, "a": 255}, "start: white"), ("fills/0/stops/1", {"r": 255, "g": 0, "b": 0, "a": 255}, "end: the hue"),
                                   ("fills/1/stops/0", {"r": 0, "g": 0, "b": 0, "a": 0}, "clear reason"), ("fills/1/stops/1", {"r": 0, "g": 0, "b": 0, "a": 255}, "black reason")])
        self.assertTrue(all(a["sid"] == rest["id"] for a in low.allow))

    def test_a_gradient_makes_no_bind_intent_for_the_fill(self):
        low, _ = lower(box(fills=[ir.grad(LTR)]))
        self.assertEqual([i for i in low.intents if i["key"].startswith("fills")], [])

    def test_a_solid_fill_is_unchanged(self):
        low, rest = lower(box(fill="color/bg"))
        self.assertEqual(rest["fills"][0]["type"], "SOLID")
        self.assertIn(("fills/0", "color/bg"), {(i["key"], i["var"]) for i in low.intents})
        self.assertEqual(low.allow, [])

    def test_an_ellipse_takes_gradient_layers_too(self):
        low, rest = lower(ir.F("Frame", [ir.E("Disc", 20, 20, fills=[ir.grad(LTR)])], dir="H"))
        disc = rest["children"][0]
        self.assertEqual((disc["type"], disc["fills"][0]["type"]), ("ELLIPSE", "GRADIENT_LINEAR"))
        self.assertEqual(len(low.allow), 2)

    def test_the_corner_radius_of_a_gradient_frame_binds_as_before(self):
        low, rest = lower(box(fills=[ir.grad(LTR)], radius="radius/control"))
        self.assertEqual(rest["cornerRadius"], low.value("radius/control"))
        self.assertEqual(len([i for i in low.intents if i["key"].endswith("Radius")]), 4)


if __name__ == "__main__":
    unittest.main()
