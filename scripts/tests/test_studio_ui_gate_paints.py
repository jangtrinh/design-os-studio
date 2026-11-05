"""G2 (bound) on paints and effects that are not a plain solid: gradient stops bind per stop, an image fill has no variable,
an invisible effect is not a finding, and instance children of a component are walked like any other node.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_paints
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_fixtures import GREY, with_node  # noqa: E402
from test_studio_ui_gate_bound import B, BoundBase  # noqa: E402

DARKER = dict(GREY, r=0.0)


def gradient(*colours):
    stops = [{"pos": float(i), "color": c} for i, c in enumerate(colours)]
    handles = [{"x": 0.0, "y": 0.5}, {"x": 1.0, "y": 0.5}, {"x": 0.0, "y": 1.0}]
    return {"kind": {"LinearGradient": {"handles": handles, "stops": stops}}, "opacity": 1.0, "visible": True, "blend": "Normal"}


class GradientTest(BoundBase):
    def drop_solid_binding(self):
        del self.bindings()["fills/0"]

    def test_a_gradient_bound_per_stop_passes(self):
        with_node(self.doc, fills=[gradient(GREY, GREY)])
        self.drop_solid_binding()
        self.bindings(**{"fills/0/stops/0": "s.bg", "fills/0/stops/1": "s.bg"})
        self.assertEqual(self.bound(), [])

    def test_an_unbound_stop_fails_and_names_the_stop(self):
        with_node(self.doc, fills=[gradient(GREY, GREY)])
        self.drop_solid_binding()
        self.bindings(**{"fills/0/stops/0": "s.bg"})
        self.assertEqual(self.bound(), [f"{B}: fills/0/stops/1 is not bound"])

    def test_a_stop_must_equal_its_variable(self):
        with_node(self.doc, fills=[gradient(GREY, DARKER)])
        self.drop_solid_binding()
        self.bindings(**{"fills/0/stops/0": "s.bg", "fills/0/stops/1": "s.bg"})
        self.assertTrue(any("fills/0/stops/1" in f and "differs from variable" in f for f in self.bound()), self.bound())

    def test_a_stroke_gradient_binds_per_stop_too(self):
        with_node(self.doc, stroke={"paints": [gradient(GREY, GREY)], "weight": 4.0, "side_weights": None})
        self.bindings(**{"strokeWeight": "s.fact"})
        self.assertEqual(sorted(f.split(": ")[1] for f in self.bound()), ["strokes/0/stops/0 is not bound", "strokes/0/stops/1 is not bound"])

    def test_an_image_fill_has_no_variable_and_is_not_a_finding(self):
        image = {"kind": {"Image": {"image_ref": "abc", "scale_mode": "Fill"}}, "opacity": 1.0, "visible": True, "blend": "Normal"}
        with_node(self.doc, fills=[image])
        self.drop_solid_binding()
        self.assertEqual(self.bound(), [])


class EffectTest(BoundBase):
    def shadow(self, visible):
        return [{"DropShadow": {"color": GREY, "offset": {"x": 0, "y": 2}, "radius": 4.0, "spread": 0.0, "visible": visible}}]

    def test_an_invisible_unbound_shadow_is_not_a_finding(self):
        with_node(self.doc, effects=self.shadow(False))
        self.assertEqual(self.bound(), [])

    def test_the_same_shadow_visible_is_a_finding(self):
        with_node(self.doc, effects=self.shadow(True))
        self.assertEqual(self.bound(), [f"{B}: effects/0/color is not bound"])


if __name__ == "__main__":
    unittest.main()
